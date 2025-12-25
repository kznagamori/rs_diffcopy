# rs_diffcopy

[Japanese](README.md)

A CLI tool that compares two directories and extracts only the files with differences.

## Features

- Extracts diff files while maintaining directory structure
- Fast and accurate file comparison using BLAKE3 hash
- **Parallel processing** for fast comparison and copying (using rayon)
- Phase-based progress display for visibility into processing status
- Displays added, modified, and deleted files in tree format
- Detailed symlink status display (added/deleted/changed/broken)
- `git apply` compatible patch file generation
- Excel report output (3-sheet layout: Summary/File Tree/Details)
- TOML config file support for reusable settings
- **Safety features**: Dangerous path protection and deletion confirmation with `--force`
- Japanese path support (no garbled characters on Windows console)
- Cross-platform (Windows / Linux / macOS)

## Installation

### Install via Cargo

```bash
cargo install --git https://github.com/kznagamori/rs_diffcopy.git
```

### Build from Source

```bash
git clone https://github.com/kznagamori/rs_diffcopy.git
cd rs_diffcopy
cargo build --release
```

### Static Linking Build (Linux)

Build a statically linked binary that works on systems with older glibc:

```bash
# Install musl target and tools
rustup target add x86_64-unknown-linux-musl
sudo apt install musl-tools  # Ubuntu/Debian

# Build with static linking
cargo build --release --target x86_64-unknown-linux-musl

# Binary is generated at:
# target/x86_64-unknown-linux-musl/release/rs_diffcopy
```

## Usage

### Basic Usage

```bash
rs_diffcopy -S <source_dir> -T <target_dir> -O <output_dir>
```

```bash
# Example: Compare old_version and new_version, output diff to output
rs_diffcopy -S old_version -T new_version -O output

# Long option names also work
rs_diffcopy --source old_version --target new_version --output output
```

### Options

| Option | Description |
|--------|-------------|
| `-S, --source <PATH>` | Source directory (required) |
| `-T, --target <PATH>` | Target directory (required) |
| `-O, --output <PATH>` | Output directory for diff files (required) |
| `-c, --config <PATH>` | Config file (TOML format) |
| `-e, --exclude <PATTERN>` | Exclude patterns (glob format, can be specified multiple times) |
| `-f, --force` | Delete output directory and re-run |
| `-s, --summary <PATH>` | Output summary to file |
| `-v, --verbose` | Verbose mode |
| `-n, --dry-run` | Show target files without copying |
| `-b, --both-versions` | Copy both old and new versions of modified files (.old/.new extensions) |
| `-P, --check-permissions <MODE>` | Check permission changes (none/scripts/all) |
| `-p, --patch` | Generate individual patch files (.patch) for modified files |
| `-F, --patch-file <PATH>` | Generate combined patch file with all changes |
| `-E, --excel <PATH>` | Output summary to Excel file (.xlsx) |
| `-h, --help` | Show help |
| `-V, --version` | Show version |

### Examples

```bash
# Basic usage (summary to stdout)
rs_diffcopy -S old_version -T new_version -O output

# Save summary to file
rs_diffcopy -S old -T new -O output -s summary.txt

# Specify exclude patterns (multiple allowed)
rs_diffcopy -S old -T new -O output -e "*.log" -e "node_modules/**"

# Force re-run (delete output directory first)
rs_diffcopy -S old -T new -O output --force

# Dry-run (preview only, no copy)
rs_diffcopy -S old -T new -O output --dry-run

# Verbose mode
rs_diffcopy -S old -T new -O output --verbose

# Copy both old and new versions of modified files
rs_diffcopy -S old -T new -O output --both-versions

# Check permission changes (script files only)
rs_diffcopy -S old -T new -O output -P scripts

# Check permission changes (all files)
rs_diffcopy -S old -T new -O output --check-permissions all

# Generate individual patch files (one .patch file per modified file)
rs_diffcopy -S old -T new -O output --patch

# Generate combined patch file (all changes in one file)
rs_diffcopy -S old -T new -O output -F changes.patch

# Generate both individual and combined patches
rs_diffcopy -S old -T new -O output -p -F all.patch

# Output Excel report
rs_diffcopy -S old -T new -O output -E report.xlsx

# Use config file
rs_diffcopy --config ./diffcopy.toml
```

## Config File (TOML Format)

Use a config file to make complex settings reusable.

### Config File Example (diffcopy.toml)

```toml
# Required settings
source = "./old_version"
target = "./new_version"
output = "./diff_output"

# Optional settings
force = false
verbose = false
dry_run = false
both_versions = false
summary = "./summary.txt"
check_permissions = "none"  # none / scripts / all
patch = false               # Generate individual patch files
patch_file = ""             # Combined patch file path (empty to disable)
excel = ""                  # Excel report output path (empty to disable)

# Exclude patterns (multiple can be specified)
exclude = [
    "*.log",
    "*.tmp",
    "node_modules/**",
    ".git/**",
    "__pycache__/**"
]
```

### Config File Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `source` | string | Yes | Source directory |
| `target` | string | Yes | Target directory |
| `output` | string | Yes | Output directory |
| `exclude` | array | - | List of exclude patterns |
| `force` | bool | - | Delete output and re-run |
| `verbose` | bool | - | Verbose mode |
| `dry_run` | bool | - | Dry run |
| `both_versions` | bool | - | Copy both versions |
| `summary` | string | - | Summary output file |
| `check_permissions` | string | - | Permission check mode (none/scripts/all) |
| `patch` | bool | - | Generate individual patch files |
| `patch_file` | string | - | Combined patch file path |
| `excel` | string | - | Excel report output path |

### Priority

When both command-line arguments and config file are specified, **command-line arguments take precedence**.

## Output Example

### Summary Output

```
rs_diffcopy Summary
================
Source: /path/to/source
Target: /path/to/target
Output: /path/to/output
Date: 2025-12-18 10:30:00

Options:
  Mode: Dry-run (no files copied)
  Copy mode: Both versions (.old/.new)
  Permission check: scripts
  Config file: diffcopy.toml
  Exclude patterns:
    - *.log
    - __pycache__
    - node_modules

Added:      5 files, 1 dir
Modified:   8 files
Deleted:    2 files, 1 dir
--------------------------
Total:     16 items

================
File Tree
================
.
├── src/
│   ├── main.rs [modified]
│   ├── new_feature.rs [added]
│   └── old_module.rs [deleted]
├── docs/ [added]
├── config.toml [modified]
└── legacy.rs [deleted]

================
Added Files
================
Directories:
  docs/

Files:
  src/new_feature.rs

================
Modified Files
================
  src/main.rs
  config.toml

================
Deleted Files
================
Files:
  src/old_module.rs
  legacy.rs
```

**Note:** The Options section and detail sections are only displayed when applicable items exist.

### Status Tags

| Tag | Meaning |
|-----|---------|
| `[added]` | Newly added |
| `[modified]` | Content changed |
| `[deleted]` | Deleted (not copied) |
| `[symlink: added]` | Newly added symbolic link |
| `[symlink: added, broken]` | Broken symbolic link |
| `[symlink: deleted]` | Deleted symbolic link |
| `[symlink: changed]` | Symbolic link with changed target |
| `[permission denied]` | Permission error (skipped) |

## Specifications

### File Comparison

- Fast comparison using BLAKE3 hash
- Compares file size first, calculates hash only if sizes match

### File Status Handling

| Status | Handling |
|--------|----------|
| New file | Copy to output |
| Modified file | Copy to output (from target) |
| New directory (including empty) | Create in output |
| Deleted file/directory | Report in summary only |
| Symbolic link | Report in summary only |

### --both-versions Mode

When using `--both-versions` option, both old and new versions of modified files are copied:

```
output/
├── file.txt.old      # Old version (from source)
├── file.txt.new      # New version (from target)
└── new_file.txt      # Added files are copied as-is
```

### Permission Check Mode

The `-P/--check-permissions` option detects file permission changes.

| Mode | Description |
|------|-------------|
| `none` | No check (default) |
| `scripts` | Check script files only |
| `all` | Check all files |

**Extensions checked in scripts mode:**
`.sh`, `.bash`, `.zsh`, `.py`, `.rb`, `.pl`, `.js`, `.php`, `.ps1`, `.bat`, `.cmd`, etc.

When permission changes are detected, they appear in the summary:

```
================
Permission Changes
================
scripts/build.sh: 755 -> 644
src/main.py: 755 -> 644
```

### Progress Display

Progress is displayed by phase:

```
[1/5] Scanning directories...
Found 1234 items.
[2/5] Comparing: [=============>              ] 45% (555/1234)
Compared 1234 items.
[3/5] Copying files...
Copied 100 files.
[4/5] Generating patches...
Generated 50 patches.
[5/5] Writing summary...
Done.
```

**Processing Phases:**
| Phase | Processing | Parallel |
|-------|------------|:--------:|
| Phase 1 | Scanning | - |
| Phase 2 | Comparing | Yes |
| Phase 3 | Copying | Yes |
| Phase 4 | Patches | - |
| Phase 5 | Summary | - |

Note: Phase 4 is only shown when `--patch` or `--patch-file` is specified

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success (with differences) |
| 1 | Error |
| 2 | Success (no differences) |

## Character Encoding

| Condition | Encoding |
|-----------|----------|
| Windows + Console | CP932 (Shift-JIS) |
| Windows + Pipe/Redirect | UTF-8 |
| Linux/macOS | UTF-8 |
| Summary file (-s) | UTF-8 |

Japanese paths are handled correctly without garbled characters on Windows console.

## License

MIT License

## Contributing

Please report bugs and feature requests to [Issues](https://github.com/kznagamori/rs_diffcopy/issues).
