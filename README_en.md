# rs_diffcopy

[Japanese](README.md)

A CLI tool that compares two directories and extracts only the files with differences.

## Features

- Extracts diff files while maintaining directory structure
- Fast and accurate file comparison using BLAKE3 hash
- **Three-way diff**: Compare base/ours/theirs directories and detect conflicts
- **Parallel processing** for fast comparison and copying (using rayon)
- Phase-based progress display for visibility into processing status
- Displays added, modified, and deleted files in tree format
- **Copy deleted files**: Extract files that exist only in source with `.deleted` extension
- **Preserve timestamps**: Keep file modification times when copying
- Detailed symlink status display (added/deleted/changed/broken)
- `git apply` compatible patch file generation
- Excel report output (3-sheet layout: Summary/File Tree/Details)
- TOML config file support for reusable settings
- **Safety features**: Dangerous path protection and deletion confirmation with `--force`
- **Robust error handling**: Continues processing when special files or copy failures occur, errors recorded in summary
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
| `-L, --excel-fold-level <LEVEL>` | Excel file tree fold level |
| `-u, --show-unchanged` | Show unchanged files in summary details |
| `-C, --save-config <PATH>` | Save current options to a config file (TOML format) |
| `-3, --three-way` | Enable three-way comparison mode |
| `-B, --base <PATH>` | Base (common ancestor) directory (required in three-way mode) |
| `-M, --merge-style <STYLE>` | Copy style for conflicts (all/ours/theirs) |
| `--conflict-only` | Output only files with conflicts |
| `--filter-status <STATUS>` | Show only files with specified status (can specify multiple) |
| `--stats-only` | Show only statistics (hide File Tree and detail sections) |
| `--no-tree` | Hide File Tree section |
| `--no-details` | Hide detail sections (Added/Modified/Deleted Files, etc.) |
| `--copy-deleted` | Copy deleted files with `.deleted` extension |
| `--preserve-timestamps` | Preserve file timestamps when copying |
| `-h, --help` | Show help |
| `-V, --version` | Show version |

> **Note for Three-way Comparison Mode:**
> - In three-way mode (`-3`), `-S/--source` is treated as "ours" (your changes) and `-T/--target` as "theirs" (their changes)
> - In three-way mode, `-S`, `-T`, and `-O` are still required, plus `-B/--base` becomes required

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

# Output Excel report (fold levels 2 and deeper)
rs_diffcopy -S old -T new -O output -E report.xlsx -L 2

# Show unchanged files in summary details
rs_diffcopy -S old -T new -O output --show-unchanged

# Save current options to config file (diff operation is also executed)
rs_diffcopy -S old -T new -O output -e "*.log" --save-config diffcopy.toml

# Use config file
rs_diffcopy --config ./diffcopy.toml

# Show only added files
rs_diffcopy -S old -T new -O output --filter-status added

# Show added and modified files (comma-separated)
rs_diffcopy -S old -T new -O output --filter-status added,modified

# Show everything except unchanged (all + ^exclusion)
rs_diffcopy -S old -T new -O output --filter-status all,^unchanged

# Show only statistics
rs_diffcopy -S old -T new -O output --stats-only

# Hide detail sections
rs_diffcopy -S old -T new -O output --no-details

# Copy deleted files with .deleted extension
rs_diffcopy -S old -T new -O output --copy-deleted

# Preserve timestamps when copying
rs_diffcopy -S old -T new -O output --preserve-timestamps

# Combine copy deleted and preserve timestamps
rs_diffcopy -S old -T new -O output --copy-deleted --preserve-timestamps
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
# excel_fold_level = 2      # Excel file tree fold level (no folding if omitted)
show_unchanged = false      # Show unchanged files in summary details

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
| `source` | string | Yes | Source directory (ours in three-way mode) |
| `target` | string | Yes | Target directory (theirs in three-way mode) |
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
| `excel_fold_level` | integer | - | Excel file tree fold level |
| `show_unchanged` | bool | - | Show unchanged files in summary details |
| `filter_status` | array | - | Status to display (supports all, ^exclusion) |
| `stats_only` | bool | - | Show only statistics |
| `no_tree` | bool | - | Hide File Tree section |
| `no_details` | bool | - | Hide detail sections |
| `copy_deleted` | bool | - | Copy deleted files with `.deleted` extension |
| `preserve_timestamps` | bool | - | Preserve file timestamps when copying |
| `three_way` | bool | - | Enable three-way comparison mode |
| `base` | string | * | Base (common ancestor) directory (required in three-way mode) |
| `merge_style` | string | - | Copy style for conflicts (all/ours/theirs) |
| `conflict_only` | bool | - | Output only files with conflicts |

> **Note:** `base` is required only when `three_way = true`

### Priority

When both command-line arguments and config file are specified, **command-line arguments take precedence**.

### Saving Config File (--save-config)

Use the `-C, --save-config <PATH>` option to save current command-line options to a config file.

```bash
# Run diff and save options to config file
rs_diffcopy -S old -T new -O output -e "*.log" -C diffcopy.toml

# Use saved config for subsequent runs
rs_diffcopy --config diffcopy.toml
```

**Features:**
- Saves config file after executing diff operation
- Includes helpful Japanese comments
- `dry_run` is always commented out (to prevent accidental activation)
- Unspecified options are included as commented samples

## Exclude Patterns (Glob Format)

The `-e`/`--exclude` option accepts glob-format patterns.

### Basic Patterns

| Pattern | Description | Match Examples |
|---------|-------------|----------------|
| `*.log` | Files with `.log` extension | `debug.log`, `src/app.log` |
| `*.tmp` | Files with `.tmp` extension | `cache.tmp`, `data/temp.tmp` |
| `test.*` | Files starting with `test.` | `test.txt`, `test.json` |

### Excluding Directories

| Pattern | Description | Match Examples |
|---------|-------------|----------------|
| `tmp` | Directory/file named `tmp` (at any level) | `tmp`, `src/tmp`, `src/tmp/file.txt` |
| `test/tmp` | Only `test/tmp` itself (contents not excluded) | `test/tmp` |
| `test/tmp/**` | All files under `test/tmp` | `test/tmp/a.txt`, `test/tmp/sub/b.txt` |

### Path Component Matching

**Important**: Simple patterns (without `/`) match against each path component (directory name/file name).

```bash
# "tmp" matches all of the following
rs_diffcopy -S old -T new -O output -e "tmp"
# Matches: tmp, src/tmp, src/tmp/file.txt, lib/tmp/data

# "test/tmp" matches only test/tmp (contents not excluded)
rs_diffcopy -S old -T new -O output -e "test/tmp"
# Matches: test/tmp
# Does NOT match: test/tmp/file.txt (not excluded, will be processed)

# "test/tmp/**" matches everything under test/tmp
rs_diffcopy -S old -T new -O output -e "test/tmp/**"
# Matches: test/tmp/file.txt, test/tmp/sub/data.txt
# Does NOT match: test/tmp (directory itself not excluded)

# To exclude both test/tmp and its contents
rs_diffcopy -S old -T new -O output -e "test/tmp" -e "test/tmp/**"
```

### Common Pattern Examples

```bash
# Exclude log files
-e "*.log"

# Exclude node_modules directory (at any level)
-e "node_modules"

# Exclude .git directory and its contents
-e ".git" -e ".git/**"

# Exclude Python cache
-e "__pycache__" -e "*.pyc"

# Exclude build outputs
-e "build" -e "dist" -e "target"

# Exclude files under a specific subdirectory
-e "vendor/cache/**"
```

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
Unchanged:  50 files
--------------------------
Total:     66 items

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

### Output Format by Destination

The output format differs between two-way and three-way comparison, and between console and file output.

| Mode | Console Output | File Output (-s option) |
|------|---------------|------------------------|
| Two-way | Tree format | Tree format |
| Three-way | Tree format (compact indicators) | Tree format (aligned indicators) |

**Three-way comparison console output (compact indicator format):**
```
Legend: [Base|Ours|Theirs] ○=exists -=missing ==same M=modified A=added D=deleted
.
├── file1.txt [○M=] ours-only
├── new_ours.txt [-A-] added-ours
├── new_theirs.txt [--A] added-theirs
├── subdir/
│   └── nested.txt [○=M] theirs-only
└── 日本語ファイル.txt [○M=] ours-only
```

**Three-way comparison file output (aligned indicator format):**
```
Legend: [Base|Ours|Theirs] ○=exists -=missing ==same M=modified A=added D=deleted
                                         B  O  T
.
├── file1.txt                          [○  M  =] ours-only
├── new_ours.txt                       [-  A  -] added-ours
├── new_theirs.txt                     [-  -  A] added-theirs
├── subdir/
│   └── nested.txt                    [○  =  M] theirs-only
└── 日本語ファイル.txt                 [○  M  =] ours-only
```

**Indicators:**
| Symbol | Meaning |
|--------|---------|
| `○` | File exists in base |
| `-` | File does not exist |
| `=` | Same as base (unchanged) |
| `M` | Modified from base |
| `A` | Added (new file) |
| `D` | Deleted |

### Status Tags

| Tag | Meaning |
|-----|---------|
| `[added]` | Newly added |
| `[modified]` | Content changed |
| `[deleted]` | Deleted (not copied) |
| `[unchanged]` | Unchanged (shown with `--show-unchanged`) |
| `[symlink: added]` | Newly added symbolic link |
| `[symlink: added, broken]` | Broken symbolic link |
| `[symlink: deleted]` | Deleted symbolic link |
| `[symlink: changed]` | Symbolic link with changed target |
| `[special: socket]` | Unix socket file (skipped) |
| `[special: fifo]` | FIFO/named pipe (skipped) |
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
| Deleted file/directory | Report in summary only (`--copy-deleted` copies with `.deleted` extension) |
| Symbolic link | Report in summary only |
| Special file (Unix) | Skip and report in summary |
| Copy failure | Skip and report in summary, processing continues |

### Special Files (Unix)

On Unix systems, special files such as sockets, FIFOs, and device files are automatically skipped and reported in the summary. This allows safe comparison of directories containing special files, such as Yocto build environments.

### --copy-deleted Mode

When using `--copy-deleted` option, deleted files (files that exist only in source) are also copied to output:

```
output/
├── file.txt          # Modified file
├── new.txt           # New file
└── old.txt.deleted   # Deleted file (with .deleted extension)
```

### --preserve-timestamps Mode

When using `--preserve-timestamps` option, file modification times (mtime) are preserved when copying. This is useful for tracking file history.

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

### Output Filter

Filter the output (console, summary file, Excel report) to show only specific items.

#### Status Filter (--filter-status)

Show only files with specified status.

**Two-way mode status values:**
| Status | Target |
|--------|--------|
| `all` | All statuses (use with exclusion) |
| `added` | Added files |
| `modified` | Modified files |
| `deleted` | Deleted files |
| `unchanged` | Unchanged files |
| `symlink` | Symbolic links |
| `special` | Special files |
| `permission` | Permission changes |
| `error` | Errors |

**Exclusion (^ prefix):**

Add `^` before a status to exclude it.

```bash
# Show everything except unchanged
--filter-status all,^unchanged

# Add added and modified, then exclude added (result: modified only)
--filter-status added,modified,^added
```

**Behavior:**
- Processed left to right (later wins)
- `all` adds all statuses to target
- `^` prefix excludes from target
- Statistics show **pre-filter totals**
- Filtered items show "(filtered out)" in statistics

#### Section Filter

| Option | Effect |
|--------|--------|
| `--stats-only` | Show only statistics (hide File Tree and detail sections) |
| `--no-tree` | Hide File Tree section |
| `--no-details` | Hide detail sections (Added/Modified/Deleted Files, etc.) |

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
| 3 | Success (with conflicts) - Three-way mode only |

### Error Handling

The following errors do not stop processing; instead, error information is recorded in the summary:

- **Permission errors**: When file read permission is denied
- **Special files**: When sockets, FIFOs, etc. are detected on Unix systems
- **Copy failures**: When file copy fails due to disk space or other reasons
- **Patch generation failures**: When patch generation fails due to file read errors

This allows large directory comparisons to continue even when some files have errors, with error information available in the summary.

## Character Encoding

| Condition | Encoding |
|-----------|----------|
| Windows + Console | CP932 (Shift-JIS) |
| Windows + Pipe/Redirect | UTF-8 |
| Linux/macOS | UTF-8 |
| Summary file (-s) | UTF-8 |

Japanese paths are handled correctly without garbled characters on Windows console.

## Three-way Comparison Mode

Three-way comparison mode compares a common ancestor (base) with two derived versions (ours/theirs) to assist with merge operations.

### Required Options

In three-way comparison mode, **all four** directory specifications are required:

| Option | Role | Description |
|--------|------|-------------|
| `-B, --base` | Base | Common ancestor (original state) |
| `-S, --source` | Ours | Your changes |
| `-T, --target` | Theirs | Their changes |
| `-O, --output` | Output | Destination for results |

> **Note:** In regular two-way comparison mode, only `-S`, `-T`, and `-O` are required. In three-way mode, `-B` is additionally required.

### Use Cases

- Pre-merge conflict detection before branch merging
- Reviewing parallel development changes
- Understanding differences in forked projects

### Basic Usage

```bash
# Run three-way comparison
rs_diffcopy --three-way -B base_dir -S my_changes -T their_changes -O output

# Short options
rs_diffcopy -3 -B base -S ours -T theirs -O output

# Extract only conflict candidates
rs_diffcopy -3 -B base -S ours -T theirs -O output --conflict-only

# With Excel report
rs_diffcopy -3 -B base -S ours -T theirs -O output -E report.xlsx
```

### File Status Classification

In three-way comparison, each file is classified into the following states:

| Status | Base | Ours | Theirs | Description |
|--------|:----:|:----:|:------:|-------------|
| `unchanged` | ○ | = | = | All three identical (no change) |
| `ours-only` | ○ | ≠ | = | Only ours modified |
| `theirs-only` | ○ | = | ≠ | Only theirs modified |
| `both-same` | ○ | ≠ | ≠(=ours) | Both made same change |
| `conflict` | ○ | ≠ | ≠ | Both made different changes (conflict) |
| `added-ours` | - | ○ | - | Added only in ours |
| `added-theirs` | - | - | ○ | Added only in theirs |
| `added-both-same` | - | ○ | ○(=ours) | Same file added in both |
| `added-both-diff` | - | ○ | ○ | Different files added in both (conflict) |
| `deleted-ours` | ○ | - | ○ | Deleted in ours |
| `deleted-theirs` | ○ | ○ | - | Deleted in theirs |
| `deleted-both` | ○ | - | - | Deleted in both |
| `modify-delete` | ○ | ≠ | - | Modified in ours, deleted in theirs (conflict) |
| `delete-modify` | ○ | - | ≠ | Deleted in ours, modified in theirs (conflict) |

Note: `○` = exists, `-` = not exists, `=` = same as base, `≠` = different from base

### Conflict Detection

The following states are marked as **conflict candidates**:

| Conflict Type | Status | Description |
|---------------|--------|-------------|
| Content conflict | `conflict` | Same file modified differently |
| Add conflict | `added-both-diff` | Same-named file added with different content |
| Modify/Delete conflict | `modify-delete` | One modified, other deleted |
| Delete/Modify conflict | `delete-modify` | One deleted, other modified |

### Merge Style (--merge-style)

| Style | Description |
|-------|-------------|
| `all` (default) | Copy all versions with `.base`/`.ours`/`.theirs` extensions for conflicts |
| `ours` | Prefer ours side for conflicts |
| `theirs` | Prefer theirs side for conflicts |

### Output Example

```
output_dir/
├── src/
│   ├── main.rs                    # ours-only: copy ours change
│   ├── utils.rs                   # theirs-only: copy theirs change
│   ├── handler.rs.base            # conflict: copy base
│   ├── handler.rs.ours            # conflict: copy ours
│   └── handler.rs.theirs          # conflict: copy theirs
└── docs/
    └── readme.md                  # added-theirs: copy theirs
```

### Summary Output Example

```
rs_diffcopy Summary (Three-way)
================================
Base:   /path/to/base
Ours:   /path/to/ours
Theirs: /path/to/theirs
Output: /path/to/output
Date:   2025-12-18 10:30:00

================
Change Matrix
================
Status          | Count
----------------|------
Unchanged       |   50
Ours only       |    8
Theirs only     |    5
Both same       |    3
Conflict        |    2
...
--------------------------
Total           |   78
Conflicts       |    4
```

### Config File (Three-way Mode)

```toml
# Three-way comparison settings
three_way = true
base = "./base_version"
source = "./my_changes"      # ours
target = "./their_changes"   # theirs
output = "./merge_output"

# Options
merge_style = "all"          # all / ours / theirs
conflict_only = false
exclude = ["*.log", ".git/**"]
```

## License

MIT License

## Contributing

Please report bugs and feature requests to [Issues](https://github.com/kznagamori/rs_diffcopy/issues).
