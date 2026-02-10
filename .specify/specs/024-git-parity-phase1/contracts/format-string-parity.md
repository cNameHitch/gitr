# Contract: Format String Parity

**Feature**: 024-git-parity-polish | **Date**: 2026-02-09

## Overview

Defines the complete set of format placeholders supported by `--format`/`--pretty` in `log` and `show` commands, and the formatting rules for built-in formats.

## Supported Placeholders

### Commit Information
| Placeholder | Description | Example |
|-------------|-------------|---------|
| `%H` | Full commit hash (40 chars) | `abc123...def456` |
| `%h` | Abbreviated commit hash | `abc1234` |
| `%T` | Full tree hash | `abc123...def456` |
| `%t` | Abbreviated tree hash | `abc1234` |
| `%P` | Full parent hashes (space-separated) | `abc123... def456...` |
| `%p` | Abbreviated parent hashes | `abc1234 def5678` |

### Author Information
| Placeholder | Description | Example |
|-------------|-------------|---------|
| `%an` | Author name | `Jane Doe` |
| `%ae` | Author email | `jane@example.com` |
| `%aE` | Author email (strict) | `jane@example.com` |
| `%ad` | Author date (respects `--date=`) | `Mon Feb 9 12:00:00 2026 +0000` |
| `%aD` | Author date, RFC2822 | `Mon, 9 Feb 2026 12:00:00 +0000` |
| `%aI` | Author date, ISO 8601 strict | `2026-02-09T12:00:00+00:00` |
| `%ai` | Author date, ISO 8601 | `2026-02-09 12:00:00 +0000` |
| `%at` | Author date, UNIX timestamp | `1739102400` |
| `%ar` | Author date, relative | `2 hours ago` |

### Committer Information
| Placeholder | Description | Example |
|-------------|-------------|---------|
| `%cn` | Committer name | `Jane Doe` |
| `%ce` | Committer email | `jane@example.com` |
| `%cd` | Committer date (respects `--date=`) | `Mon Feb 9 12:00:00 2026 +0000` |
| `%cD` | Committer date, RFC2822 | `Mon, 9 Feb 2026 12:00:00 +0000` |
| `%cI` | Committer date, ISO 8601 strict | `2026-02-09T12:00:00+00:00` |
| `%ci` | Committer date, ISO 8601 | `2026-02-09 12:00:00 +0000` |
| `%ct` | Committer date, UNIX timestamp | `1739102400` |
| `%cr` | Committer date, relative | `2 hours ago` |

### Message
| Placeholder | Description | Example |
|-------------|-------------|---------|
| `%s` | Subject (first line) | `Fix bug in parser` |
| `%b` | Body (remaining lines) | `Detailed description...` |
| `%B` | Raw body (full message) | `Fix bug in parser\n\nDetailed description...` |

### Decoration
| Placeholder | Description | Example |
|-------------|-------------|---------|
| `%d` | Ref names with parens, leading space | ` (HEAD -> main, tag: v1.0)` |
| `%D` | Ref names without parens | `HEAD -> main, tag: v1.0` |

### Miscellaneous
| Placeholder | Description | Example |
|-------------|-------------|---------|
| `%n` | Newline | `\n` |
| `%%` | Literal percent | `%` |

## Built-in Format Rules

### oneline
- Format: `<full-40-char-hash> <subject>`
- Single line, no newline between entries in multi-commit output
- Hash is FULL (40 chars), NOT abbreviated — this is a gitr bug fix

### raw
- Shows raw commit object format
- Message body indented with 4 leading spaces per line
- Header fields: `tree`, `parent` (one per parent), `author`, `committer`

### email
- Date uses no-padding for day: `9 Feb` not `09 Feb`
- Format: `From <hash> Mon Sep 17 00:00:00 2001\nFrom: <author>\nDate: <date>\nSubject: [PATCH] <subject>\n\n<body>`

## Decoration Ordering

When `--decorate` is active or `%d`/`%D` is used:
1. `HEAD ->` (if HEAD points to this commit via a branch)
2. Local branches (alphabetical)
3. Remote-tracking branches (alphabetical)
4. Tags (alphabetical)

Format: `HEAD -> main, origin/main, tag: v1.0`

## Error Handling

Unknown format placeholders (e.g., `%Z`) → output the literal `%Z` string (matching git behavior, which silently passes through unknown specifiers).
