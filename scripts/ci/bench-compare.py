#!/usr/bin/env python3
"""Compare Criterion benchmark baselines and report regressions.

Parses estimates.json files from two Criterion baselines, computes
percentage change for gitr benchmarks only, and outputs:
  - bench-results.json  (machine-readable)
  - bench-results.md    (PR comment)

Usage:
    python3 scripts/ci/bench-compare.py --base main --pr pr --threshold 5
"""

import argparse
import json
import os
import sys


def find_baselines(criterion_dir, baseline_name):
    """Walk criterion_dir and return {bench_key: estimates} for a baseline.

    Criterion stores results as:
        <group>/<bench_id>/<baseline>/estimates.json
    or for benchmarks without size variants:
        <group>/<bench_id>/<baseline>/estimates.json

    bench_key is the relative path from criterion_dir up to (but not
    including) the baseline directory, e.g. "status/gitr/small".
    """
    results = {}
    for root, dirs, files in os.walk(criterion_dir):
        if "estimates.json" not in files:
            continue
        rel = os.path.relpath(root, criterion_dir)
        parts = rel.split(os.sep)
        # Find the baseline directory in the path
        if baseline_name not in parts:
            continue
        baseline_idx = parts.index(baseline_name)
        # Everything before the baseline name is the bench key
        bench_key = os.sep.join(parts[:baseline_idx])
        if not bench_key:
            continue
        est_path = os.path.join(root, "estimates.json")
        with open(est_path) as f:
            data = json.load(f)
        results[bench_key] = data["mean"]["point_estimate"]
    return results


def is_gitr_bench(bench_key):
    """Return True if this benchmark measures gitr (not C git)."""
    parts = bench_key.split(os.sep)
    # Criterion bench IDs for gitr contain "gitr" as a path component.
    # Examples: "status/gitr/small", "init/gitr", "cat-file/gitr_-p/small"
    return any(p == "gitr" or p.startswith("gitr_") or p.startswith("gitr/") for p in parts)


def format_ns(ns):
    """Format nanoseconds into a human-readable string."""
    if ns >= 1_000_000_000:
        return f"{ns / 1_000_000_000:.2f}s"
    if ns >= 1_000_000:
        return f"{ns / 1_000_000:.2f}ms"
    if ns >= 1_000:
        return f"{ns / 1_000:.2f}us"
    return f"{ns:.0f}ns"


def main():
    parser = argparse.ArgumentParser(description="Compare Criterion baselines")
    parser.add_argument("--base", required=True, help="Base baseline name")
    parser.add_argument("--pr", required=True, help="PR baseline name")
    parser.add_argument("--threshold", type=float, default=5.0, help="Regression threshold (%%)")
    parser.add_argument("--criterion-dir", default="target/criterion", help="Criterion output dir")
    args = parser.parse_args()

    base_results = find_baselines(args.criterion_dir, args.base)
    pr_results = find_baselines(args.criterion_dir, args.pr)

    if not base_results:
        print(f"ERROR: No base baseline '{args.base}' found in {args.criterion_dir}", file=sys.stderr)
        sys.exit(1)
    if not pr_results:
        print(f"ERROR: No PR baseline '{args.pr}' found in {args.criterion_dir}", file=sys.stderr)
        sys.exit(1)

    all_keys = sorted(set(base_results.keys()) | set(pr_results.keys()))

    results = []
    for key in all_keys:
        # Only track gitr benchmarks for regressions
        if not is_gitr_bench(key):
            continue

        base_ns = base_results.get(key)
        pr_ns = pr_results.get(key)

        if base_ns is None:
            results.append({
                "benchmark": key,
                "base_ns": None,
                "pr_ns": pr_ns,
                "pct_change": None,
                "status": "NEW",
            })
        elif pr_ns is None:
            results.append({
                "benchmark": key,
                "base_ns": base_ns,
                "pr_ns": None,
                "pct_change": None,
                "status": "REMOVED",
            })
        else:
            pct = ((pr_ns - base_ns) / base_ns) * 100
            if pct > args.threshold:
                status = "REGRESSION"
            elif pct < -args.threshold:
                status = "IMPROVEMENT"
            else:
                status = "OK"
            results.append({
                "benchmark": key,
                "base_ns": base_ns,
                "pr_ns": pr_ns,
                "pct_change": round(pct, 2),
                "status": status,
            })

    regressions = [r for r in results if r["status"] == "REGRESSION"]
    improvements = [r for r in results if r["status"] == "IMPROVEMENT"]
    has_regression = len(regressions) > 0

    # --- Write JSON output ---
    json_output = {
        "has_regression": has_regression,
        "threshold": args.threshold,
        "total": len(results),
        "regressions": len(regressions),
        "improvements": len(improvements),
        "results": results,
    }
    with open("bench-results.json", "w") as f:
        json.dump(json_output, f, indent=2)

    # --- Write Markdown output ---
    status_emoji = "FAIL" if has_regression else "PASS"
    lines = []
    lines.append("## Benchmark Results")
    lines.append(f"**Threshold**: {args.threshold}% | **Status**: {status_emoji}")
    lines.append(f"### Summary: {len(results)} benchmarks, {len(regressions)} regressions, {len(improvements)} improvements")
    lines.append("")

    if regressions:
        lines.append("### Regressions")
        lines.append("")
        lines.append("| Benchmark | Base | PR | Change |")
        lines.append("|-----------|------|-----|--------|")
        for r in regressions:
            lines.append(
                f"| `{r['benchmark']}` | {format_ns(r['base_ns'])} | {format_ns(r['pr_ns'])} | +{r['pct_change']}% |"
            )
        lines.append("")

    if improvements:
        lines.append("### Improvements")
        lines.append("")
        lines.append("| Benchmark | Base | PR | Change |")
        lines.append("|-----------|------|-----|--------|")
        for r in improvements:
            lines.append(
                f"| `{r['benchmark']}` | {format_ns(r['base_ns'])} | {format_ns(r['pr_ns'])} | {r['pct_change']}% |"
            )
        lines.append("")

    new_benches = [r for r in results if r["status"] == "NEW"]
    removed_benches = [r for r in results if r["status"] == "REMOVED"]
    if new_benches:
        names = ", ".join("`" + r["benchmark"] + "`" for r in new_benches)
        lines.append(f"**New benchmarks**: {names}")
        lines.append("")
    if removed_benches:
        names = ", ".join("`" + r["benchmark"] + "`" for r in removed_benches)
        lines.append(f"**Removed benchmarks**: {names}")
        lines.append("")

    lines.append("<details>")
    lines.append("<summary>All Results</summary>")
    lines.append("")
    lines.append("| Benchmark | Base | PR | Change | Status |")
    lines.append("|-----------|------|-----|--------|--------|")
    for r in results:
        base_str = format_ns(r["base_ns"]) if r["base_ns"] is not None else "-"
        pr_str = format_ns(r["pr_ns"]) if r["pr_ns"] is not None else "-"
        if r["pct_change"] is not None:
            sign = "+" if r["pct_change"] > 0 else ""
            change_str = f"{sign}{r['pct_change']}%"
        else:
            change_str = r["status"]
        lines.append(
            f"| `{r['benchmark']}` | {base_str} | {pr_str} | {change_str} | {r['status']} |"
        )
    lines.append("")
    lines.append("</details>")
    lines.append("")

    md_content = "\n".join(lines)
    with open("bench-results.md", "w") as f:
        f.write(md_content)

    # Print summary to stdout
    print(f"Compared {len(results)} gitr benchmarks (threshold: {args.threshold}%)")
    print(f"  Regressions:  {len(regressions)}")
    print(f"  Improvements: {len(improvements)}")
    if regressions:
        print("\nRegressed benchmarks:")
        for r in regressions:
            print(f"  {r['benchmark']}: +{r['pct_change']}%")


if __name__ == "__main__":
    main()
