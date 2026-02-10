#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# gitr Parity Test Coverage Report
#
# Generates a coverage matrix showing how many flags each command defines,
# how many E2E/parity tests cover it, and the resulting coverage percentage.
#
# Usage:
#   ./scripts/ci/parity-coverage.sh              # colored table
#   ./scripts/ci/parity-coverage.sh --no-color   # plain table (for CI logs)
#   ./scripts/ci/parity-coverage.sh --json        # JSON output
#   ./scripts/ci/parity-coverage.sh --check       # exit 1 if any command has 0 tests
#
# Compatible with bash 3.2+ (macOS default) and Linux bash 4+.
# =============================================================================

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
COMMANDS_DIR="$REPO_ROOT/crates/git-cli/src/commands"
TESTS_DIR="$REPO_ROOT/crates/git-cli/tests"

# --- Parse flags -----------------------------------------------------------

USE_COLOR=true
OUTPUT_JSON=false
CHECK_MODE=false

for arg in "$@"; do
    case "$arg" in
        --no-color) USE_COLOR=false ;;
        --json)     OUTPUT_JSON=true; USE_COLOR=false ;;
        --check)    CHECK_MODE=true ;;
        --help|-h)
            echo "Usage: $0 [--no-color] [--json] [--check]"
            echo ""
            echo "  --no-color  Disable colored output (for CI)"
            echo "  --json      Output JSON instead of table"
            echo "  --check     Exit non-zero if any command has 0 tests"
            exit 0
            ;;
        *)
            echo "Unknown flag: $arg" >&2
            exit 1
            ;;
    esac
done

# --- Color helpers ---------------------------------------------------------

if $USE_COLOR; then
    C_RESET='\033[0m'
    C_BOLD='\033[1m'
    C_RED='\033[31m'
    C_GREEN='\033[32m'
    C_YELLOW='\033[33m'
    C_CYAN='\033[36m'
else
    C_RESET=''
    C_BOLD=''
    C_RED=''
    C_GREEN=''
    C_YELLOW=''
    C_CYAN=''
fi

# --- Data storage using parallel indexed arrays (bash 3.2 compatible) ------
#
# We store per-command data in parallel arrays indexed by position:
#   CMD_NAMES[i]   - command name (e.g. "cat-file")
#   CMD_FLAGS[i]   - number of #[arg(] lines
#   CMD_TESTS[i]   - total test count
#   CMD_IGNORED[i] - ignored test count

CMD_NAMES=()
CMD_FLAGS=()
CMD_TESTS=()
CMD_IGNORED=()

# --- Step 1: Extract command names and flag counts -------------------------

# Extract canonical command names from the command_name() match in mod.rs.
# This is the authoritative source for command-name <-> filename mapping.
while IFS= read -r cmd_name; do
    CMD_NAMES[${#CMD_NAMES[@]}]="$cmd_name"
done < <(
    grep '=> "' "$COMMANDS_DIR/mod.rs" \
    | sed 's/.*=> "//' \
    | sed 's/".*//' \
    | sort
)

# For each command, find the corresponding .rs file and count #[arg(] lines
for i in $(seq 0 $((${#CMD_NAMES[@]} - 1))); do
    cmd="${CMD_NAMES[$i]}"
    # Convert command name to filename: e.g. "cat-file" -> "cat_file.rs"
    filename="$(echo "$cmd" | tr '-' '_').rs"
    filepath="$COMMANDS_DIR/$filename"

    if [ -f "$filepath" ]; then
        count=$(grep -c '#\[arg(' "$filepath" 2>/dev/null) || count=0
    else
        count=0
    fi

    CMD_FLAGS[$i]=$count
done

# --- Step 2: Count tests per command ---------------------------------------

# Collect all test file paths (parity_* and e2e_*)
TEST_FILES=()
while IFS= read -r f; do
    [ -z "$f" ] && continue
    TEST_FILES[${#TEST_FILES[@]}]="$f"
done < <(find "$TESTS_DIR" -maxdepth 1 \( -name 'parity_*.rs' -o -name 'e2e_*.rs' \) | sort)

# Concatenate all test files into a single temp file for faster searching.
# Each line is prefixed with the source file path and line number via grep -n.
ALL_TESTS_TMP=$(mktemp)
trap 'rm -f "$ALL_TESTS_TMP"' EXIT

# Build a combined file with "FILENAME:LINENUM:CONTENT" for every line
for tf in "${TEST_FILES[@]}"; do
    # Using awk to prefix each line with filename:linenum:
    awk -v f="$tf" '{print f":"NR":"$0}' "$tf"
done > "$ALL_TESTS_TMP"

for i in $(seq 0 $((${#CMD_NAMES[@]} - 1))); do
    cmd="${CMD_NAMES[$i]}"
    # Convert to underscore form for matching in test function names
    cmd_pattern="$(echo "$cmd" | tr '-' '_')"

    total=0
    ignored=0

    # Find all test function lines matching this command pattern.
    # Pattern: "fn test_<cmd_pattern>" followed by underscore, open-paren, or space.
    while IFS= read -r match_line; do
        [ -z "$match_line" ] && continue
        total=$((total + 1))

        # Extract the file and line number from the match
        match_file="$(echo "$match_line" | cut -d: -f1)"
        match_linenum="$(echo "$match_line" | cut -d: -f2)"

        # Check the 3 lines before this function for #[ignore]
        start=$((match_linenum - 3))
        if [ $start -lt 1 ]; then
            start=1
        fi
        end=$((match_linenum - 1))
        if [ $end -ge 1 ]; then
            preceding=$(sed -n "${start},${end}p" "$match_file" 2>/dev/null || true)
            if echo "$preceding" | grep -q '#\[ignore'; then
                ignored=$((ignored + 1))
            fi
        fi
    done < <(grep "fn test_${cmd_pattern}[_( ]" "$ALL_TESTS_TMP" 2>/dev/null | cut -d: -f1-2 || true)

    CMD_TESTS[$i]=$total
    CMD_IGNORED[$i]=$ignored
done

# --- Step 3: Compute summary stats ----------------------------------------

total_commands=${#CMD_NAMES[@]}
total_flags=0
total_tests=0
total_ignored=0
total_active=0
commands_zero_tests=0
coverage_sum=0

for i in $(seq 0 $((total_commands - 1))); do
    flags=${CMD_FLAGS[$i]}
    tests=${CMD_TESTS[$i]}
    ign=${CMD_IGNORED[$i]}
    active=$((tests - ign))

    total_flags=$((total_flags + flags))
    total_tests=$((total_tests + tests))
    total_ignored=$((total_ignored + ign))
    total_active=$((total_active + active))

    if [ "$tests" -eq 0 ]; then
        commands_zero_tests=$((commands_zero_tests + 1))
    fi

    # Coverage: min(100, (active / max(flags, 1)) * 100)
    denom=$flags
    if [ "$denom" -lt 1 ]; then
        denom=1
    fi
    pct=$((active * 100 / denom))
    if [ "$pct" -gt 100 ]; then
        pct=100
    fi
    coverage_sum=$((coverage_sum + pct))
done

if [ "$total_commands" -gt 0 ]; then
    avg_coverage=$((coverage_sum / total_commands))
else
    avg_coverage=0
fi

# --- Step 4: Output -------------------------------------------------------

# Helper: generate a bar chart string
make_bar() {
    local pct=$1
    local width=10
    local filled=$((pct * width / 100))
    local empty=$((width - filled))
    local bar=""
    local j

    j=0
    while [ $j -lt $filled ]; do
        bar="${bar}#"
        j=$((j + 1))
    done
    j=0
    while [ $j -lt $empty ]; do
        bar="${bar}-"
        j=$((j + 1))
    done
    echo "$bar"
}

if $OUTPUT_JSON; then
    # --- JSON output -------------------------------------------------------
    echo "{"
    echo "  \"date\": \"$(date +%Y-%m-%d)\","
    echo "  \"git_version\": \"$(git --version 2>/dev/null || echo 'unknown')\","
    echo "  \"commands\": ["

    first=true
    for i in $(seq 0 $((total_commands - 1))); do
        cmd="${CMD_NAMES[$i]}"
        flags=${CMD_FLAGS[$i]}
        tests=${CMD_TESTS[$i]}
        ign=${CMD_IGNORED[$i]}
        active=$((tests - ign))
        denom=$flags
        if [ "$denom" -lt 1 ]; then denom=1; fi
        pct=$((active * 100 / denom))
        if [ "$pct" -gt 100 ]; then pct=100; fi

        if $first; then
            first=false
        else
            echo ","
        fi
        printf '    {"name": "%s", "flags": %d, "tests": %d, "ignored": %d, "active": %d, "coverage": %d}' \
            "$cmd" "$flags" "$tests" "$ign" "$active" "$pct"
    done

    echo ""
    echo "  ],"
    echo "  \"summary\": {"
    echo "    \"total_commands\": $total_commands,"
    echo "    \"total_flags\": $total_flags,"
    echo "    \"total_tests\": $total_tests,"
    echo "    \"total_active\": $total_active,"
    echo "    \"total_ignored\": $total_ignored,"
    echo "    \"commands_zero_tests\": $commands_zero_tests,"
    echo "    \"average_coverage\": $avg_coverage"
    echo "  }"
    echo "}"
else
    # --- Table output ------------------------------------------------------
    printf "${C_BOLD}=== gitr Parity Test Coverage Report ===${C_RESET}\n"
    printf "Date: %s\n" "$(date +%Y-%m-%d)"
    printf "Git version: %s\n" "$(git --version 2>/dev/null || echo 'unknown')"
    echo ""

    # Header
    printf "${C_BOLD}%-20s${C_RESET} | ${C_BOLD}%5s${C_RESET} | ${C_BOLD}%5s${C_RESET} | ${C_BOLD}%7s${C_RESET} | ${C_BOLD}%6s${C_RESET} | ${C_BOLD}%-14s${C_RESET}\n" \
        "Command" "Flags" "Tests" "Ignored" "Active" "Coverage"
    printf -- "%-20s-|-%5s-|-%5s-|-%7s-|-%6s-|-%s\n" \
        "--------------------" "-----" "-----" "-------" "------" "--------------"

    for i in $(seq 0 $((total_commands - 1))); do
        cmd="${CMD_NAMES[$i]}"
        flags=${CMD_FLAGS[$i]}
        tests=${CMD_TESTS[$i]}
        ign=${CMD_IGNORED[$i]}
        active=$((tests - ign))
        denom=$flags
        if [ "$denom" -lt 1 ]; then denom=1; fi
        pct=$((active * 100 / denom))
        if [ "$pct" -gt 100 ]; then pct=100; fi

        bar=$(make_bar "$pct")

        # Choose color based on coverage
        if [ "$pct" -ge 80 ]; then
            color="$C_GREEN"
        elif [ "$pct" -ge 40 ]; then
            color="$C_YELLOW"
        else
            color="$C_RED"
        fi

        printf "%-20s | %5d | %5d | %7d | %6d | ${color}%s %3d%%${C_RESET}\n" \
            "$cmd" "$flags" "$tests" "$ign" "$active" "$bar" "$pct"
    done

    echo ""
    printf "${C_BOLD}Summary:${C_RESET}\n"
    printf "  Commands:  ${C_CYAN}%d${C_RESET} total\n" "$total_commands"
    printf "  Flags:     ${C_CYAN}%d${C_RESET} total\n" "$total_flags"
    printf "  Tests:     ${C_CYAN}%d${C_RESET} total (${C_GREEN}%d active${C_RESET}, ${C_YELLOW}%d ignored${C_RESET})\n" \
        "$total_tests" "$total_active" "$total_ignored"

    if [ "$commands_zero_tests" -gt 0 ]; then
        printf "  0-test:    ${C_RED}%d commands${C_RESET}\n" "$commands_zero_tests"
    else
        printf "  0-test:    ${C_GREEN}%d commands${C_RESET}\n" "$commands_zero_tests"
    fi

    printf "  Coverage:  ${C_CYAN}%d%%${C_RESET} average\n" "$avg_coverage"
fi

# --- --check mode ----------------------------------------------------------

if $CHECK_MODE; then
    if [ "$commands_zero_tests" -gt 0 ]; then
        echo "" >&2
        printf "${C_RED}ERROR: %d command(s) have 0 tests:${C_RESET}\n" "$commands_zero_tests" >&2
        for i in $(seq 0 $((total_commands - 1))); do
            if [ "${CMD_TESTS[$i]}" -eq 0 ]; then
                printf "  - %s\n" "${CMD_NAMES[$i]}" >&2
            fi
        done
        exit 1
    fi
fi
