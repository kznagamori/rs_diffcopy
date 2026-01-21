# rs_diffcopy

[Japanese (日本語)](README.md)

A CLI tool for comparing directories and extracting changed files while preserving the directory structure.
Supports both two-way and three-way (merge) comparisons.

## Features

- **Two-way Comparison**: Compare source and target directories, detecting added, modified, and deleted files
- **Three-way Comparison**: Compare base, ours, and theirs directories, detecting conflicts
- **Structure Preservation**: Maintains original directory structure in output
- **Unicode Path Support**: Correctly handles file paths containing Unicode characters (including Japanese)
- **Patch Generation**: Generates unified diff format patches for changed files
- **Excel Reports**: Outputs comparison results as Excel files (.xlsx)
- **Parallel Processing**: Fast processing of large numbers of files
- **Configuration Files**: Reusable TOML format configuration files

## Installation

### Build from Source

```bash
git clone https://github.com/kznagamori/rs_diffcopy.git
cd rs_diffcopy
cargo build --release
```

The built binary will be located at `target/release/rs_diffcopy`.

### Install via Cargo

```bash
cargo install --git https://github.com/kznagamori/rs_diffcopy.git
```

## Basic Usage

### Two-way Comparison

```bash
# Basic usage
rs_diffcopy -S <source> -T <target> -O <output>

# Example: Compare old and new directories, output to diff directory
rs_diffcopy -S ./old -T ./new -O ./diff
```

### Three-way Comparison

```bash
# Three-way comparison (merge)
rs_diffcopy --three-way -B <base> -S <ours> -T <theirs> -O <output>

# Example: Compare changes from common ancestor
rs_diffcopy --three-way -B ./base -S ./ours -T ./theirs -O ./merged
```

## All Options

| Option | Short | Description |
|--------|-------|-------------|
| `--source` | `-S` | Source directory |
| `--target` | `-T` | Target directory |
| `--output` | `-O` | Output directory |
| `--config` | `-c` | Configuration file path |
| `--exclude` | `-e` | Exclude patterns (glob format, can be specified multiple times) |
| `--force` | `-f` | Force overwrite output directory |
| `--summary` | `-s` | Save summary to file |
| `--verbose` | `-v` | Verbose output mode (show file names during processing) |
| `--dry-run` | `-n` | Dry run (don't actually copy files) |
| `--both-versions` | `-b` | Copy both old and new versions (.old/.new) |
| `--check-permissions <MODE>` | `-P` | Permission check (none/scripts/all) |
| `--patch` | `-p` | Generate individual patch files |
| `--patch-file` | `-F` | Generate combined patch file |
| `--excel` | `-E` | Generate Excel report |
| `--excel-fold-level <LEVEL>` | `-L` | Excel file tree fold level |
| `--show-unchanged` | `-u` | Show unchanged files |
| `--save-config <PATH>` | `-C` | Save current options to config file |
| `--filter-status <STATUS>` | | Status filter (comma-separated, `^` to exclude) |
| `--stats-only` | | Show only header/options/statistics |
| `--no-tree` | | Hide File Tree section |
| `--no-details` | | Hide details sections |
| `--copy-deleted` | | Copy deleted files (.deleted) |
| `--preserve-timestamps` | | Preserve file timestamps |
| `--workers <NUM>` | `-j` | Number of parallel workers |
| `--temp-dir <PATH>` | | Temporary directory for intermediate files |
| `--color <MODE>` | | Color output (auto/always/never) |
| `--log-level <LEVEL>` | | Log level (error/warn/info/debug) |
| `--three-way` | `-3` | Three-way comparison mode |
| `--base` | `-B` | Base directory for three-way comparison |
| `--merge-style` | `-M` | Merge style (all/ours/theirs) |
| `--conflict-only` | | Output only conflicts |
| `--help` | `-h` | Show help |
| `--version` | `-V` | Show version |

> Note: `--stats-only` affects **console output only**. `--summary` and `--excel` still contain the full report.

For the allowed filter values and group keywords, see "Output filter" in `rs_diffcopy.md`.

## Output Examples

### Two-way Comparison Output

```
╔═════════════════════════════════════════════════════════════════════════════╗
║                              rs_diffcopy Summary                            ║
╠═════════════════════════════════════════════════════════════════════════════╣
║  Source:    /path/to/old                                                    ║
║  Target:    /path/to/new                                                    ║
║  Output:    /path/to/diff                                                   ║
║  Date:      2026-01-08 22:30:00                                             ║
╚═════════════════════════════════════════════════════════════════════════════╝

┌─────────────────────────────────────────────────────────────────────────────┐
│  Statistics                                                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│  Added:       3 files                                                       │
│  Modified:    5 files                                                       │
│  Deleted:     2 files                                                       │
│  Unchanged:   10 files                                                      │
│  Total:       20 items                                                      │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Three-way Comparison Output

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  Three-way Comparison Statistics                                            │
├─────────────────────────────────────────────────────────────────────────────┤
│  Ours only:      2 files                                                    │
│  Theirs only:    3 files                                                    │
│  Both same:      1 file                                                     │
│  CONFLICT:       2 files                                                    │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Differences found (normal completion) |
| 1 | Error occurred |
| 2 | No differences |
| 3 | Conflicts found (three-way comparison) |

## Configuration File

You can use a TOML format configuration file.
For application-wide `settings.toml`, see "Application settings file (settings.toml)" in `rs_diffcopy.md`.

```toml
# config.toml
source = "/path/to/source"
target = "/path/to/target"
output = "/path/to/output"
exclude = ["*.log", "node_modules", "__pycache__"]
both_versions = true
patch = true
verbose = false
```

Using a configuration file:

```bash
rs_diffcopy --config config.toml
```

Saving current options to a configuration file:

```bash
rs_diffcopy -S ./old -T ./new -O ./diff -e "*.log" --save-config config.toml
```

## Advanced Usage

### Exclude Patterns

```bash
# Exclude log files and node_modules
rs_diffcopy -S ./old -T ./new -O ./diff -e "*.log" -e "node_modules"
```

### Output and Reports

```bash
# Save summary to file
rs_diffcopy -S ./old -T ./new -O ./diff --summary summary.txt

# Generate Excel report
rs_diffcopy -S ./old -T ./new -O ./diff --excel report.xlsx
```

### Patch Generation

```bash
# Generate individual patch files
rs_diffcopy -S ./old -T ./new -O ./diff --patch

# Generate combined patch file
rs_diffcopy -S ./old -T ./new -O ./diff --patch-file changes.patch

# Generate both
rs_diffcopy -S ./old -T ./new -O ./diff --patch --patch-file changes.patch
```

### Copy Options

```bash
# Copy both old and new versions of modified files
rs_diffcopy -S ./old -T ./new -O ./diff --both-versions

# Copy deleted files
rs_diffcopy -S ./old -T ./new -O ./diff --copy-deleted

# Preserve timestamps when copying
rs_diffcopy -S ./old -T ./new -O ./diff --preserve-timestamps
```

### Filters and Display Control

```bash
# Show only added and modified (copy targets are also filtered)
rs_diffcopy -S ./old -T ./new -O ./diff --filter-status added,modified

# Show everything except unchanged (implicit all + exclusion)
rs_diffcopy -S ./old -T ./new -O ./diff --filter-status ^unchanged

# Show everything except added and deleted (all + exclusions)
rs_diffcopy -S ./old -T ./new -O ./diff --filter-status all,^added,^deleted

# Show only added (aliases also accepted: add/a)
rs_diffcopy -S ./old -T ./new -O ./diff --filter-status add

# Show only errors and symlinks
rs_diffcopy -S ./old -T ./new -O ./diff --filter-status error,symlink

# Show statistics only (copy still runs)
rs_diffcopy -S ./old -T ./new -O ./diff --stats-only

# Hide File Tree section
rs_diffcopy -S ./old -T ./new -O ./diff --no-tree

# Hide details sections
rs_diffcopy -S ./old -T ./new -O ./diff --no-details
```

> Note: `--stats-only` affects **console output only**. `--summary` and `--excel` still contain the full report.

### Color and Logging

```bash
# Always use colored output
rs_diffcopy -S ./old -T ./new -O ./diff --color always

# Set log level to debug
rs_diffcopy -S ./old -T ./new -O ./diff --log-level debug
```

### Performance Tuning

```bash
# Set number of workers
rs_diffcopy -S ./old -T ./new -O ./diff --workers 4

# Set temp directory
rs_diffcopy -S ./old -T ./new -O ./diff --temp-dir /tmp/rs_diffcopy
```

### Permission Checks

```bash
# Check permission changes for script files only
rs_diffcopy -S ./old -T ./new -O ./diff --check-permissions scripts

# Check permission changes for all files
rs_diffcopy -S ./old -T ./new -O ./diff --check-permissions all
```

### Saving a Config File

```bash
# Save current options to a config file (diff processing still runs)
rs_diffcopy -S ./old -T ./new -O ./diff -e "*.log" --save-config diffcopy.toml
```

### Three-way Comparison

```bash
# Output conflicts only
rs_diffcopy --three-way -B ./base -S ./ours -T ./theirs -O ./output --conflict-only

# Show conflicts only (group keyword)
rs_diffcopy --three-way -B ./base -S ./ours -T ./theirs -O ./output --filter-status conflicts

# Show only added (group keyword expands)
rs_diffcopy --three-way -B ./base -S ./ours -T ./theirs -O ./output --filter-status added

# Exclude added (implicit all + exclusion)
rs_diffcopy --three-way -B ./base -S ./ours -T ./theirs -O ./output --filter-status ^added

# Prefer ours
rs_diffcopy --three-way -B ./base -S ./ours -T ./theirs -O ./output --merge-style ours

# Prefer theirs
rs_diffcopy --three-way -B ./base -S ./ours -T ./theirs -O ./output --merge-style theirs
```

## Requirements

- Rust 1.70 or later
- Supported OS: Linux, macOS, Windows

## License

MIT License

Copyright (c) 2024 kznagamori

## Author

kznagamori

## Links

- [GitHub Repository](https://github.com/kznagamori/rs_diffcopy)
- [Issues](https://github.com/kznagamori/rs_diffcopy/issues)
