# rs_diffcopy

[English](README_en.md)

2つのディレクトリを比較し、差分があるファイルのみを抽出するCLIツールです。

## 特徴

- 差分ファイルを階層構造を維持したまま別フォルダへ抽出
- BLAKE3ハッシュによる高速かつ正確なファイル比較
- **三者間比較（Three-way diff）**: base/ours/theirsの3ディレクトリを比較してコンフリクト検出
- **並列処理**による高速な比較・コピー（rayon使用）
- フェーズ別進捗表示で処理状況を可視化
- 追加・変更・削除ファイルをツリー形式で表示
- **削除ファイルのコピー**: source側にのみ存在するファイルを`.deleted`拡張子付きで抽出
- **タイムスタンプ保持**: コピー時にファイルの更新日時を保持
- シンボリックリンクの詳細な状態表示（追加/削除/変更/壊れたリンク）
- `git apply`互換のパッチファイル生成
- Excelレポート出力（Summary/File Tree/Detailsの3シート構成）
- TOML設定ファイルによる複雑な設定の再利用
- **安全機能**: `--force`時の危険なパス保護と削除確認
- **堅牢なエラー処理**: 特殊ファイルやコピー失敗時も処理継続、エラーはサマリーに記録
- 日本語パス対応（Windowsコンソールでも文字化けなし）
- クロスプラットフォーム（Windows / Linux / macOS）

## インストール

### Cargoでインストール

```bash
cargo install --git https://github.com/kznagamori/rs_diffcopy.git
```

### ソースからビルド

```bash
git clone https://github.com/kznagamori/rs_diffcopy.git
cd rs_diffcopy
cargo build --release
```

### スタティックリンクビルド（Linux）

古いglibcのシステムでも動作するスタティックリンク版をビルドできます：

```bash
# musl ターゲットとツールのインストール
rustup target add x86_64-unknown-linux-musl
sudo apt install musl-tools  # Ubuntu/Debian

# スタティックリンクでビルド
cargo build --release --target x86_64-unknown-linux-musl

# バイナリは以下に生成
# target/x86_64-unknown-linux-musl/release/rs_diffcopy
```

## 使い方

### 基本的な使い方

```bash
rs_diffcopy -S <比較元> -T <比較先> -O <出力先>
```

```bash
# 例: old_version と new_version を比較し、差分を output に出力
rs_diffcopy -S old_version -T new_version -O output

# 長いオプション名も使用可能
rs_diffcopy --source old_version --target new_version --output output
```

### オプション

| オプション | 説明 |
|-----------|------|
| `-S, --source <PATH>` | 比較元ディレクトリ（必須）|
| `-T, --target <PATH>` | 比較先ディレクトリ（必須）|
| `-O, --output <PATH>` | 差分出力先ディレクトリ（必須）|
| `-c, --config <PATH>` | 設定ファイル（TOML形式）|
| `-e, --exclude <PATTERN>` | 除外パターン（glob形式、複数指定可） |
| `-f, --force` | 出力先を全削除して再実行 |
| `-s, --summary <PATH>` | サマリーをファイルに出力 |
| `-v, --verbose` | 詳細出力モード |
| `-n, --dry-run` | 実際にコピーせず対象ファイルを表示 |
| `-b, --both-versions` | 変更ファイルの新旧両方をコピー（.old/.new拡張子付与） |
| `-P, --check-permissions <MODE>` | 権限変更をチェック（none/scripts/all） |
| `-p, --patch` | 変更ファイルごとに個別のパッチファイル(.patch)を生成 |
| `-F, --patch-file <PATH>` | 全変更を統合したパッチファイルを生成 |
| `-E, --excel <PATH>` | サマリーをExcelファイル(.xlsx)に出力 |
| `-L, --excel-fold-level <LEVEL>` | Excelファイルツリーの折りたたみレベル |
| `-u, --show-unchanged` | 変更がないファイルをサマリー詳細に表示 |
| `-C, --save-config <PATH>` | 現在のオプションを設定ファイル(TOML形式)に保存 |
| `-3, --three-way` | 三者間比較モードを有効化 |
| `-B, --base <PATH>` | 共通祖先ディレクトリ（三者間モード時必須） |
| `-M, --merge-style <STYLE>` | コンフリクト時のコピー方式（all/ours/theirs） |
| `--conflict-only` | コンフリクトのあるファイルのみ出力 |
| `--filter-status <STATUS>` | 指定ステータスのファイルのみ表示（複数指定可） |
| `--stats-only` | 統計情報のみ表示（File Tree、詳細セクションを非表示） |
| `--no-tree` | File Treeセクションを非表示 |
| `--no-details` | 詳細セクション（Added/Modified/Deleted Files等）を非表示 |
| `--copy-deleted` | 削除ファイルもコピー（.deleted拡張子付与） |
| `--preserve-timestamps` | コピー時にファイルのタイムスタンプを保持 |
| `-h, --help` | ヘルプ表示 |
| `-V, --version` | バージョン表示 |

> **三者間比較モード時の注意:**
> - 三者間比較モード（`-3`）では、`-S/--source` が「ours」（自分の変更）、`-T/--target` が「theirs」（相手の変更）として扱われます
> - 三者間比較モードでも `-S`、`-T`、`-O` は引き続き必須です。加えて `-B/--base` も必須となります

### 使用例

```bash
# 基本使用（サマリーは標準出力）
rs_diffcopy -S old_version -T new_version -O output

# サマリーをファイルに保存
rs_diffcopy -S old -T new -O output -s summary.txt

# 除外パターン指定（複数可）
rs_diffcopy -S old -T new -O output -e "*.log" -e "node_modules/**"

# 強制再実行（出力先を削除して実行）
rs_diffcopy -S old -T new -O output --force

# ドライラン（確認のみ、コピーしない）
rs_diffcopy -S old -T new -O output --dry-run

# 詳細出力モード
rs_diffcopy -S old -T new -O output --verbose

# 変更ファイルの新旧両方をコピー（.old/.new拡張子付与）
rs_diffcopy -S old -T new -O output --both-versions

# 権限変更をチェック（スクリプトファイルのみ）
rs_diffcopy -S old -T new -O output -P scripts

# 権限変更をチェック（すべてのファイル）
rs_diffcopy -S old -T new -O output --check-permissions all

# 個別のパッチファイルを生成（変更ファイルごとに.patchファイル作成）
rs_diffcopy -S old -T new -O output --patch

# 統合パッチファイルを生成（全変更を1ファイルに）
rs_diffcopy -S old -T new -O output -F changes.patch

# 個別と統合の両方を生成
rs_diffcopy -S old -T new -O output -p -F all.patch

# Excelレポートを出力
rs_diffcopy -S old -T new -O output -E report.xlsx

# Excelレポートを出力（深さ2以上を折りたたみ）
rs_diffcopy -S old -T new -O output -E report.xlsx -L 2

# 変更がないファイルもサマリー詳細に表示
rs_diffcopy -S old -T new -O output --show-unchanged

# 現在のオプションを設定ファイルに保存（差分処理も実行される）
rs_diffcopy -S old -T new -O output -e "*.log" --save-config diffcopy.toml

# 設定ファイルを使用
rs_diffcopy --config ./diffcopy.toml

# 追加ファイルのみ表示
rs_diffcopy -S old -T new -O output --filter-status added

# 追加と変更ファイルのみ表示（カンマ区切り）
rs_diffcopy -S old -T new -O output --filter-status added,modified

# unchanged以外すべて表示（all + ^除外）
rs_diffcopy -S old -T new -O output --filter-status all,^unchanged

# 統計情報のみ表示
rs_diffcopy -S old -T new -O output --stats-only

# 詳細セクションを非表示
rs_diffcopy -S old -T new -O output --no-details

# 削除ファイルもコピー（.deleted拡張子付与）
rs_diffcopy -S old -T new -O output --copy-deleted

# タイムスタンプを保持してコピー
rs_diffcopy -S old -T new -O output --preserve-timestamps

# 削除ファイルのコピーとタイムスタンプ保持を組み合わせ
rs_diffcopy -S old -T new -O output --copy-deleted --preserve-timestamps
```

## 設定ファイル（TOML形式）

設定ファイルを使用することで、複雑な設定を再利用可能にします。

### 設定ファイルの例（diffcopy.toml）

```toml
# 必須設定
source = "./old_version"
target = "./new_version"
output = "./diff_output"

# オプション設定
force = false
verbose = false
dry_run = false
both_versions = false
summary = "./summary.txt"
check_permissions = "none"  # none / scripts / all
patch = false               # 個別パッチファイル生成
patch_file = ""             # 統合パッチファイルパス（空で無効）
excel = ""                  # Excelレポート出力パス（空で無効）
# excel_fold_level = 2      # Excelファイルツリーの折りたたみレベル（省略時は折りたたみなし）
show_unchanged = false      # 変更がないファイルをサマリー詳細に表示

# 除外パターン（複数指定可）
exclude = [
    "*.log",
    "*.tmp",
    "node_modules/**",
    ".git/**",
    "__pycache__/**"
]
```

### 設定ファイルの項目

| 項目 | 型 | 必須 | 説明 |
|------|------|------|------|
| `source` | string | ✅ | 比較元ディレクトリ（三者間モードではours） |
| `target` | string | ✅ | 比較先ディレクトリ（三者間モードではtheirs） |
| `output` | string | ✅ | 出力先ディレクトリ |
| `exclude` | array | - | 除外パターンのリスト |
| `force` | bool | - | 出力先を削除して再実行 |
| `verbose` | bool | - | 詳細出力モード |
| `dry_run` | bool | - | ドライラン |
| `both_versions` | bool | - | 新旧両方をコピー |
| `summary` | string | - | サマリー出力先ファイル |
| `check_permissions` | string | - | 権限チェックモード（none/scripts/all） |
| `patch` | bool | - | 個別パッチファイル生成 |
| `patch_file` | string | - | 統合パッチファイルパス |
| `excel` | string | - | Excelレポート出力パス |
| `excel_fold_level` | integer | - | Excelファイルツリーの折りたたみレベル |
| `show_unchanged` | bool | - | 変更がないファイルをサマリー詳細に表示 |
| `filter_status` | array | - | 表示するステータス（all, ^除外対応） |
| `stats_only` | bool | - | 統計情報のみ表示 |
| `no_tree` | bool | - | File Treeセクション非表示 |
| `no_details` | bool | - | 詳細セクション非表示 |
| `copy_deleted` | bool | - | 削除ファイルもコピー（.deleted拡張子） |
| `preserve_timestamps` | bool | - | コピー時にタイムスタンプを保持 |
| `three_way` | bool | - | 三者間比較モードを有効化 |
| `base` | string | ※ | 共通祖先ディレクトリ（三者間モード時は必須） |
| `merge_style` | string | - | コンフリクト時のコピー方式（all/ours/theirs） |
| `conflict_only` | bool | - | コンフリクトのあるファイルのみ出力 |

> **注:** `base` は `three_way = true` の場合のみ必須です

### 優先順位

コマンドライン引数と設定ファイルの両方が指定された場合、**コマンドライン引数が優先**されます。

### 設定ファイルの保存（--save-config）

`-C, --save-config <PATH>` オプションを使用すると、現在のコマンドラインオプションを設定ファイルとして保存できます。

```bash
# 差分処理を実行し、オプションを設定ファイルに保存
rs_diffcopy -S old -T new -O output -e "*.log" -C diffcopy.toml

# 次回以降は設定ファイルを使用
rs_diffcopy --config diffcopy.toml
```

**特徴：**
- 差分処理を実行した後、設定ファイルを保存
- 日本語コメント付きで分かりやすい
- `dry_run`は常にコメントアウト（誤って有効にならないよう配慮）
- 未指定オプションはコメントアウトされたサンプルとして記載

## 除外パターン（glob形式）

`-e`/`--exclude`オプションで指定する除外パターンはglob形式です。

### 基本パターン

| パターン | 説明 | マッチ例 |
|----------|------|---------|
| `*.log` | 拡張子が`.log`のファイル | `debug.log`, `src/app.log` |
| `*.tmp` | 拡張子が`.tmp`のファイル | `cache.tmp`, `data/temp.tmp` |
| `test.*` | `test.`で始まるファイル | `test.txt`, `test.json` |

### ディレクトリの除外

| パターン | 説明 | マッチ例 |
|----------|------|---------|
| `tmp` | 名前が`tmp`のディレクトリ/ファイル（どの階層でも） | `tmp`, `src/tmp`, `src/tmp/file.txt` |
| `test/tmp` | `test/tmp`のみ（中身は除外されない） | `test/tmp` |
| `test/tmp/**` | `test/tmp`配下の全ファイル | `test/tmp/a.txt`, `test/tmp/sub/b.txt` |

### パスコンポーネントマッチング

**重要**: 単純なパターン（`/`を含まない）は、パスの各コンポーネント（ディレクトリ名・ファイル名）に対してマッチします。

```bash
# "tmp" は以下すべてにマッチ
rs_diffcopy -S old -T new -O output -e "tmp"
# マッチ: tmp, src/tmp, src/tmp/file.txt, lib/tmp/data

# "test/tmp" は test/tmp のみにマッチ（中身は除外されない）
rs_diffcopy -S old -T new -O output -e "test/tmp"
# マッチ: test/tmp
# 非マッチ: test/tmp/file.txt（除外されず処理対象）

# "test/tmp/**" は test/tmp 配下すべてにマッチ
rs_diffcopy -S old -T new -O output -e "test/tmp/**"
# マッチ: test/tmp/file.txt, test/tmp/sub/data.txt
# 非マッチ: test/tmp（ディレクトリ自体は除外されない）

# test/tmp とその中身両方を除外したい場合
rs_diffcopy -S old -T new -O output -e "test/tmp" -e "test/tmp/**"
```

### よく使うパターン例

```bash
# ログファイルを除外
-e "*.log"

# node_modules ディレクトリを除外（どの階層でも）
-e "node_modules"

# .git ディレクトリとその中身を除外
-e ".git" -e ".git/**"

# Python キャッシュを除外
-e "__pycache__" -e "*.pyc"

# ビルド出力を除外
-e "build" -e "dist" -e "target"

# 特定のサブディレクトリ配下を除外
-e "vendor/cache/**"
```

## 出力例

### サマリー出力

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

**注意:** Optionsセクションおよび各詳細セクションは、該当する項目がある場合のみ表示されます。

### 出力先別の形式

二者間比較と三者間比較で、コンソール出力とファイル出力の形式が異なります。

| モード | コンソール出力 | ファイル出力（-s指定時） |
|--------|---------------|------------------------|
| 二者間比較 (File Tree) | ツリー形式 | ツリー形式 |
| 三者間比較 (File Tree) | ツリー形式（コンパクト） | ツリー形式（整列） |

**三者間比較のコンソール出力例（コンパクト形式）:**
```
Legend: [Base|Ours|Theirs] ○=exists -=missing ==same M=modified A=added D=deleted
.
├── file1.txt [○M=] ours-only
├── subdir/
│   └── nested.txt [○=M] theirs-only
└── 日本語ファイル.txt [○M=] ours-only
```

**三者間比較のファイル出力例（整列形式）:**
```
Legend: [Base|Ours|Theirs] ○=exists -=missing ==same M=modified A=added D=deleted
                                     B  O  T
.
├── file1.txt                      [○  M  =] ours-only
├── subdir/
│   └── nested.txt                [○  =  M] theirs-only
└── 日本語ファイル.txt             [○  M  =] ours-only
```

### ステータスタグ

| タグ | 意味 |
|------|------|
| `[added]` | 新規追加 |
| `[modified]` | 変更あり |
| `[deleted]` | 削除（コピーされない） |
| `[unchanged]` | 変更なし（`--show-unchanged`時に表示） |
| `[symlink: added]` | 新規追加されたシンボリックリンク |
| `[symlink: added, broken]` | 壊れたシンボリックリンク |
| `[symlink: deleted]` | 削除されたシンボリックリンク |
| `[symlink: changed]` | リンク先が変更されたシンボリックリンク |
| `[special: socket]` | Unixソケットファイル（スキップ） |
| `[special: fifo]` | FIFO/名前付きパイプ（スキップ） |
| `[permission denied]` | 権限エラー（スキップ） |

## 動作仕様

### ファイル比較

- BLAKE3ハッシュによる高速比較
- まずファイルサイズを比較し、同じ場合のみハッシュを計算

### ファイル状態別の扱い

| 状態 | 扱い |
|------|------|
| 新規ファイル | 出力先にコピー |
| 変更ファイル | 出力先にコピー（比較先のファイル） |
| 新規ディレクトリ（空含む） | 出力先に作成 |
| 削除ファイル/ディレクトリ | サマリーに記載のみ（`--copy-deleted`時は`.deleted`拡張子付きでコピー） |
| シンボリックリンク | サマリーに記載のみ |
| 特殊ファイル（Unix） | スキップしてサマリーに記載 |
| コピー失敗 | スキップしてサマリーに記載、処理は継続 |

### --copy-deleted モード

削除されたファイル（source側にのみ存在するファイル）も出力先にコピーします：

```
output/
├── file.txt          # 変更ファイル
├── new.txt           # 新規ファイル
└── old.txt.deleted   # 削除ファイル（.deleted拡張子付与）
```

### --preserve-timestamps モード

`--preserve-timestamps` オプションを使用すると、コピー時にファイルの更新日時（mtime）を保持します。ファイル履歴の追跡に有用です。

### 特殊ファイル（Unix）

Unix系OSでは、ソケット、FIFO、デバイスファイルなどの特殊ファイルは自動的にスキップされ、サマリーに記載されます。これにより、Yoctoビルド環境など特殊ファイルを含むディレクトリの比較も安全に実行できます。

### --both-versions モード

`--both-versions` オプションを使用すると、変更ファイルの新旧両方をコピーします：

```
output/
├── file.txt.old      # 変更前のファイル（source側）
├── file.txt.new      # 変更後のファイル（target側）
└── new_file.txt      # 新規追加ファイルはそのまま
```

### 権限チェックモード

`-P/--check-permissions` オプションでファイルの権限変更を検出できます。

| モード | 説明 |
|--------|------|
| `none` | チェックしない（デフォルト） |
| `scripts` | スクリプトファイルのみチェック |
| `all` | すべてのファイルをチェック |

**scriptsモードの対象拡張子：**
`.sh`, `.bash`, `.zsh`, `.py`, `.rb`, `.pl`, `.js`, `.php`, `.ps1`, `.bat`, `.cmd` など

権限変更が検出された場合、サマリーに以下のように表示されます：

```
================
Permission Changes
================
scripts/build.sh: 755 -> 644
src/main.py: 755 -> 644
```

### 出力フィルター機能

出力結果（コンソール、サマリーファイル、Excelレポート）に対して、表示内容をフィルタリングできます。

#### ステータスフィルター（--filter-status）

指定したステータスのファイルのみを表示します。

**二者間モード用ステータス値：**
| ステータス値 | 対象 |
|-------------|------|
| `all` | 全ステータス（除外指定と組み合わせて使用） |
| `added` | 追加されたファイル |
| `modified` | 変更されたファイル |
| `deleted` | 削除されたファイル |
| `unchanged` | 変更なしのファイル |
| `symlink` | シンボリックリンク |
| `special` | 特殊ファイル |
| `permission` | 権限変更 |
| `error` | エラー |

**除外指定（^プレフィックス）：**

`^`をステータス値の前に付けることで、そのステータスを除外できます。

```bash
# unchanged以外すべて表示
--filter-status all,^unchanged

# addedとmodifiedを追加し、addedを除外（結果: modifiedのみ）
--filter-status added,modified,^added
```

**動作仕様：**
- 指定は左から右へ順番に処理（後勝ち）
- `all` を指定すると全ステータスを対象に追加
- `^`プレフィックス付きは対象から除外
- 統計情報は**フィルター前の全体数**を表示
- フィルター適用時は統計情報に「(filtered out)」を表示

#### セクションフィルター

| オプション | 効果 |
|-----------|------|
| `--stats-only` | 統計情報のみ表示（File Tree、詳細セクションを非表示） |
| `--no-tree` | File Treeセクションを非表示 |
| `--no-details` | 詳細セクション（Added/Modified/Deleted Files等）を非表示 |

### 進捗表示

処理はフェーズ別に進捗表示されます：

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

**処理フェーズ:**
| フェーズ | 処理内容 | 並列化 |
|---------|---------|:------:|
| Phase 1 | Scanning | - |
| Phase 2 | Comparing | 並列 |
| Phase 3 | Copying | 並列 |
| Phase 4 | Patches | - |
| Phase 5 | Summary | - |

※ Phase 4 は `--patch` または `--patch-file` 指定時のみ

### 終了コード

| コード | 意味 |
|--------|------|
| 0 | 正常終了（差分あり） |
| 1 | エラー終了 |
| 2 | 正常終了（差分なし） |
| 3 | 正常終了（コンフリクトあり）※三者間モードのみ |

### エラーハンドリング

以下のエラーは処理を停止せず、サマリーにエラー情報を記録して処理を継続します：

- **権限エラー**: ファイルの読み取り権限がない場合
- **特殊ファイル**: Unix系OSでソケット、FIFO等が検出された場合
- **コピー失敗**: ディスク容量不足等でファイルコピーに失敗した場合
- **パッチ生成失敗**: ファイル読み込みエラー等でパッチ生成に失敗した場合

これにより、大規模なディレクトリ比較で一部のファイルにエラーがあっても、他のファイルの処理は継続され、エラー情報はサマリーで確認できます。

## 文字エンコーディング

| 条件 | エンコーディング |
|------|-----------------|
| Windows + コンソール | CP932 (Shift-JIS) |
| Windows + パイプ/リダイレクト | UTF-8 |
| Linux/macOS | UTF-8 |
| サマリーファイル (-s) | UTF-8 |

日本語パスを正しく処理し、Windowsコンソールでも文字化けしません。

## 三者間比較モード（Three-way diff）

三者間比較モードは、共通の祖先（base）と2つの派生バージョン（ours/theirs）を比較し、マージ作業を支援します。

### 必須オプション

三者間比較モードでは、以下の4つのディレクトリ指定が**すべて必須**です：

| オプション | 役割 | 説明 |
|-----------|------|------|
| `-B, --base` | Base | 共通の祖先（変更前の状態） |
| `-S, --source` | Ours | 自分の変更版 |
| `-T, --target` | Theirs | 相手の変更版 |
| `-O, --output` | Output | 結果の出力先 |

> **注:** 通常の二者間比較モードでは `-S`、`-T`、`-O` の3つが必須ですが、三者間比較モードではさらに `-B` が必要です。

### ユースケース

- ブランチマージ前のコンフリクト事前確認
- 複数人で並行開発した変更の統合確認
- フォークしたプロジェクトの差分把握

### 基本的な使い方

```bash
# 三者間比較を実行
rs_diffcopy --three-way -B base_dir -S my_changes -T their_changes -O output

# 短いオプション
rs_diffcopy -3 -B base -S ours -T theirs -O output

# コンフリクト候補のみ抽出
rs_diffcopy -3 -B base -S ours -T theirs -O output --conflict-only

# Excelレポート付き
rs_diffcopy -3 -B base -S ours -T theirs -O output -E report.xlsx
```

### ファイル状態の判定

三者間比較では、各ファイルを以下の状態に分類します：

| 状態 | Base | Ours | Theirs | 説明 |
|------|:----:|:----:|:------:|------|
| `unchanged` | ○ | = | = | 3つとも同一（変更なし） |
| `ours-only` | ○ | ≠ | = | oursのみ変更 |
| `theirs-only` | ○ | = | ≠ | theirsのみ変更 |
| `both-same` | ○ | ≠ | ≠(=ours) | 両方が同じ変更 |
| `conflict` | ○ | ≠ | ≠ | 両方が異なる変更（コンフリクト候補） |
| `added-ours` | - | ○ | - | oursでのみ追加 |
| `added-theirs` | - | - | ○ | theirsでのみ追加 |
| `added-both-same` | - | ○ | ○(=ours) | 両方で同じファイルを追加 |
| `added-both-diff` | - | ○ | ○ | 両方で異なるファイルを追加（コンフリクト） |
| `deleted-ours` | ○ | - | ○ | oursで削除 |
| `deleted-theirs` | ○ | ○ | - | theirsで削除 |
| `deleted-both` | ○ | - | - | 両方で削除 |
| `modify-delete` | ○ | ≠ | - | oursで変更、theirsで削除（コンフリクト） |
| `delete-modify` | ○ | - | ≠ | oursで削除、theirsで変更（コンフリクト） |

※ `○` = 存在、`-` = 存在しない、`=` = baseと同一、`≠` = baseと異なる

### コンフリクト判定

以下の状態は**コンフリクト候補**として特別にマークされます：

| コンフリクト種別 | 状態 | 説明 |
|-----------------|------|------|
| 内容コンフリクト | `conflict` | 同じファイルを異なる内容に変更 |
| 追加コンフリクト | `added-both-diff` | 同名ファイルを異なる内容で追加 |
| 変更/削除コンフリクト | `modify-delete` | 一方が変更、他方が削除 |
| 削除/変更コンフリクト | `delete-modify` | 一方が削除、他方が変更 |

### マージスタイル（--merge-style）

| スタイル | 説明 |
|----------|------|
| `all`（デフォルト） | コンフリクト時は全バージョンを`.base`/`.ours`/`.theirs`拡張子付きでコピー |
| `ours` | コンフリクト時はours側を優先 |
| `theirs` | コンフリクト時はtheirs側を優先 |

### 出力例

```
output_dir/
├── src/
│   ├── main.rs                    # ours-only: oursの変更をコピー
│   ├── utils.rs                   # theirs-only: theirsの変更をコピー
│   ├── handler.rs.base            # conflict: baseをコピー
│   ├── handler.rs.ours            # conflict: oursをコピー
│   └── handler.rs.theirs          # conflict: theirsをコピー
└── docs/
    └── readme.md                  # added-theirs: theirsをコピー
```

### サマリー出力例

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

### 設定ファイル（三者間モード）

```toml
# 三者間比較の設定
three_way = true
base = "./base_version"
source = "./my_changes"      # ours
target = "./their_changes"   # theirs
output = "./merge_output"

# オプション
merge_style = "all"          # all / ours / theirs
conflict_only = false
exclude = ["*.log", ".git/**"]
```

## ライセンス

MIT License

## 貢献

バグ報告や機能要望は [Issues](https://github.com/kznagamori/rs_diffcopy/issues) へお願いします。
