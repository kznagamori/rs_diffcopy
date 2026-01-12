# rs_diffcopy 要件定義

## 1. プロジェクト概要

- **アプリケーション名**: `rs_diffcopy`
- **バージョン**: `v1.0.0`
- **開発目的**: diffコマンドだと初心者には変更点が分かりにくいため、初心者・非技術者でも変更点が視覚的に分かるツールを作成
- **ゴール**: 2つのディレクトリを比較し、差異があるファイルのみを階層構造を維持したまま別フォルダへ抽出する
- **想定ユーザー**: 初心者、非技術者

---

## 2. 対象プラットフォーム・開発環境

| 項目 | 内容 |
|------|------|
| 対象OS | Windows 10/11 (x64), Ubuntu 22+ (x64), macOS |
| 開発言語 | Rust |
| インターフェース | CLI |
| 配布形態 | GitHub経由で `cargo install`、またはzipで配布 |

---

## 3. 機能一覧（v1.0.0）

| 機能 | 説明 |
|------|------|
| ディレクトリ比較 | 2つのディレクトリを比較し、差分ファイルを抽出 |
| 三者間比較 | base/ours/theirsの3ディレクトリを比較してコンフリクト検出 |
| 除外パターン | glob形式で除外ファイル/ディレクトリを指定可能 |
| 設定ファイル | TOML形式の設定ファイルで複雑な設定を再利用 |
| 新旧両方コピー | 変更ファイルの新旧両方を `.old`/`.new` 拡張子付きでコピー |
| 削除ファイルのコピー | source側にのみ存在するファイルを`.deleted`拡張子付きで出力 |
| タイムスタンプ保持 | コピー時にファイルの更新日時を保持 |
| 権限チェック | ファイル権限の変更を検出（スクリプト限定/全ファイル） |
| パッチ生成 | 変更ファイルのunified diff形式パッチを生成（`git apply`互換） |
| Excelレポート | サマリーをExcelファイル(.xlsx)として出力（3シート構成） |
| 変更なしファイル表示 | 変更がないファイルをサマリー詳細に表示可能 |
| 出力フィルター | ステータス別フィルタ、セクション表示制御 |
| ドライラン | 実際にコピーせず、対象ファイルをプレビュー |
| 進捗表示 | フェーズ別プログレスバーを表示 |
| 並列処理 | ファイル比較・コピーを並列実行で高速化 |
| カラー出力 | ターミナルで色付き出力（added=緑, deleted=赤, modified=黄）。`--color`オプションで制御 |
| 安全機能 | 危険なパス（システムディレクトリ等）の誤削除を防止、削除前に確認 |

---

## 4. アプリケーション設定ファイル（settings.toml）

CLIアプリケーションと同じディレクトリに `settings.toml` を配置することで、アプリケーション全体のデフォルト設定を行えます。

### 4.1 設定ファイルの配置場所

| OS | 配置場所 |
|----|---------|
| Windows | `rs_diffcopy.exe` と同じディレクトリ |
| Linux/macOS | `rs_diffcopy` バイナリと同じディレクトリ |

### 4.2 設定ファイルの例

```toml
# settings.toml（アプリケーションと同じディレクトリに配置）

# ===================
# キャッシュ設定
# ===================
# 一時ファイルの保存先ディレクトリ（省略時はシステムのtempディレクトリ）
# temp_dir = "/tmp/rs_diffcopy"

# ===================
# パフォーマンス設定
# ===================
# 並列処理のワーカー数（省略時は論理CPUコア数）
# 低スペックPCでは小さい値を設定して負荷を軽減
# workers = 4

# ===================
# 出力設定
# ===================
# カラー出力のデフォルト設定
# "auto": 端末判定で自動切替（デフォルト）
# "always": 常にカラー出力
# "never": 常にカラーなし
# color = "auto"

# ログ出力レベル
# "error": エラーのみ
# "warn": 警告以上（デフォルト）
# "info": 情報以上
# "debug": デバッグ情報を含む全て
# log_level = "warn"

# ===================
# デフォルト除外パターン
# ===================
# 全ての比較で適用されるデフォルトの除外パターン
# 入力設定ファイル（-c）のexcludeと併用される
# default_exclude = [
#     ".git/**",
#     "node_modules/**",
#     "__pycache__/**",
#     "*.pyc",
#     ".DS_Store",
#     "Thumbs.db"
# ]
```

### 4.3 設定項目一覧

| 項目 | 型 | デフォルト | 説明 |
|------|------|------------|------|
| `temp_dir` | string | システムtempディレクトリ | 一時ファイルの保存先 |
| `workers` | integer | 論理CPUコア数 | 並列処理のワーカー数（1以上） |
| `color` | string | `"auto"` | カラー出力設定（`auto`/`always`/`never`） |
| `log_level` | string | `"warn"` | ログ出力レベル（`error`/`warn`/`info`/`debug`） |
| `default_exclude` | array | `[]` | デフォルトの除外パターン（全比較に適用） |

### 4.4 設定の優先順位

各設定項目は以下の優先順位で解決されます（上が最優先）：

1. コマンドライン引数（`-e`, `--workers` 等）
2. 入力設定ファイル（`-c, --config` で指定）
3. アプリケーション設定ファイル（`settings.toml`）
4. デフォルト値

### 4.5 除外パターンの合成

`default_exclude`（settings.toml）と`exclude`（入力設定ファイル/-eオプション）は**合成**されます：

```toml
# settings.toml
default_exclude = ["node_modules/**", ".git/**"]

# diffcopy.toml（または -e オプション）
exclude = ["*.log", "dist/**"]

# 実際に適用される除外パターン:
# - node_modules/**
# - .git/**
# - *.log
# - dist/**
```

※ `settings.toml` が存在しない場合、または各項目が記載されていない場合は、デフォルト値が使用されます。

---

## 5. CLI仕様

### 5.1 コマンドライン引数

```
rs_diffcopy [OPTIONS]

必須オプション（設定ファイル未使用時）:
  -S, --source <PATH>              比較元ディレクトリ
  -T, --target <PATH>              比較先ディレクトリ
  -O, --output <PATH>              差分ファイルの出力先

オプション:
  -c, --config <PATH>              設定ファイル（TOML形式）
  -e, --exclude <PATTERN>          除外パターン（複数指定可、glob形式）
  -f, --force                      出力先を全削除して再実行（削除前に確認プロンプト表示）
  -s, --summary <PATH>             サマリーをファイルに出力
  -v, --verbose                    詳細出力モード（処理中ファイル名を表示）
  -n, --dry-run                    実際にコピーせず、対象ファイルを表示
  -b, --both-versions              変更ファイルの新旧両方をコピー（.old/.new拡張子付与）
  -P, --check-permissions <MODE>   権限変更をチェック（none/scripts/all、デフォルト: none）
  -p, --patch                      変更ファイルごとに個別パッチファイル(.patch)を生成（`--dry-run` 時は生成しない）
  -F, --patch-file <PATH>          全変更を統合したパッチファイルを生成（`--dry-run` 時は生成しない）
  -E, --excel <PATH>               サマリーをExcelファイル(.xlsx)に出力
  -L, --excel-fold-level <LEVEL>   Excelファイルツリーの折りたたみレベル（指定深さ以上を折りたたみ）
  -u, --show-unchanged             変更がないファイルをサマリー詳細に表示
  -C, --save-config <PATH>         現在のオプションを設定ファイル(TOML形式)に保存
  --filter-status <STATUS>         指定ステータスのファイルのみコピー/表示（複数指定可: カンマ区切り）
  --stats-only                     ヘッダー/Options/統計情報のみ表示（--no-tree と --no-details の両方を指定した場合と同じ）
  --no-tree                        File Treeセクションを非表示
  --no-details                     詳細セクション（Added/Modified/Deleted Files等）を非表示
  --copy-deleted                   削除ファイルもコピー（.deleted拡張子付与）
  --preserve-timestamps            コピー時にファイルのタイムスタンプを保持
  -j, --workers <NUM>              並列処理のワーカー数（デフォルト: CPUコア数）
      --temp-dir <PATH>            一時ファイルの保存先ディレクトリ
      --color <MODE>               カラー出力（auto/always/never、デフォルト: auto）
      --log-level <LEVEL>          ログ出力レベル（error/warn/info/debug、デフォルト: warn）
  -h, --help                       ヘルプ表示
  -V, --version                    バージョン表示

三者間モード専用オプション:
  -3, --three-way                  三者間比較モードを有効化
  -B, --base <PATH>                共通祖先ディレクトリ
  -M, --merge-style <STYLE>        コンフリクト時のコピー方式（all/ours/theirs）
      --conflict-only              コンフリクト候補のみ出力
```

#### オプションの補足説明

**`--verbose` と `--log-level` の違い**

| オプション | 用途 | 出力先 |
|-----------|------|--------|
| `--verbose` | 処理中のファイル名をリアルタイム表示 | 標準エラー出力（プログレスと同時表示） |
| `--log-level` | アプリケーション内部のログ出力レベル | 標準エラー出力 |

- `--verbose`は進捗表示の詳細化（処理中ファイル名の表示）に使用
- `--log-level`はデバッグや問題調査用のログ出力制御に使用
- 両方を同時に指定可能

**`--dry-run` の動作**

ドライランモードでは以下の処理が**実行されます**：
- ディレクトリ走査、ファイル比較
- サマリー生成・表示

以下の処理は**スキップされます**：
- ファイルのコピー（出力ディレクトリへの書き込み）
- パッチファイルの生成

ファイルコピー処理がスキップされるため、実際の変更を適用するよりも**迅速に差分結果を確認できます**。

コピー系オプション（`--copy-deleted` / `--both-versions` / `--preserve-timestamps`）は
ドライラン時は反映されません。

サマリーには `Mode: Dry run (no files copied)` と表示されます。

**`--filter-status` の許容値**

指定可能なステータスは以下です（大文字小文字は区別しない）:

受理値（推奨）:
- `added`
- `modified`
- `deleted`
- `unchanged`（`--show-unchanged` が必要）
- `symlink`
- `special`
- `error`
- `permission`

別名:
- `add` / `a`
- `modify` / `m`
- `delete` / `d`
- `same` / `u`（`--show-unchanged` が必要）
- `sym` / `link`
- `spec`
- `err`
- `perm`

※ CLIのヘルプには受理値（推奨）のみを掲載し、別名は仕様書内の互換情報として扱う

※ `--filter-status` 指定時は、コピー対象とサマリー詳細（File Tree/Details等）に同じフィルターが適用される
※ 統計情報は**フィルター前の全体数**を表示し、除外されたステータスには `(filtered out)` を付与する
※ フィルター後の件数は `Showing: N items (filtered)` として表示する

**`--summary` / `--excel` の上書き動作**

出力先ファイルが既に存在する場合は、確認なしで上書きされる。

**`--force` と `--dry-run` の確認プロンプト**

`--dry-run` 時は出力先ディレクトリの削除は行われないため、`--force` の確認プロンプトは表示されない。

**`--no-tree` / `--no-details` の影響**

これらはサマリー表示の制御のみであり、ファイルコピーは通常通り実行される。

**`--stats-only` の影響**

このオプションは**コンソール出力にのみ影響**し、ヘッダー/Options/統計情報のみを表示します（`--no-tree` と `--no-details` の組み合わせと同じ）。
ファイル出力（`--summary` や `--excel`）が同時に指定されている場合、それらのファイルには**完全なサマリー（ファイルリスト等を含む）が出力されます**。
**注意:** このオプションは表示内容を限定するだけであり、ファイルコピーなどの処理は通常通り実行されます。コピーを実行せずに結果のみをプレビューしたい場合は `--dry-run` を使用してください。

**`--summary` の出力先**

`--summary` で指定したファイルにサマリーを出力します。コンソール出力は `--stats-only` / `--no-tree` / `--no-details` / `--filter-status` の指定に従います。

**`--summary` と `--excel` の併用**

両方指定した場合は、テキストサマリーとExcelレポートをそれぞれ出力する。

### 5.2 使用例

```bash
# 基本使用
rs_diffcopy -S old_version -T new_version -O output

# 長いオプション名
rs_diffcopy --source old_version --target new_version --output output

# サマリーをファイルに保存
rs_diffcopy -S old -T new -O output -s ./summary.txt

# 除外パターン指定（複数可）
rs_diffcopy -S old -T new -O output -e "*.log" -e "node_modules/**"

# 強制再実行（出力先を削除して実行）
rs_diffcopy -S old -T new -O output --force

# ドライラン（確認のみ）
rs_diffcopy -S old -T new -O output --dry-run

# 変更ファイルの新旧両方をコピー
rs_diffcopy -S old -T new -O output --both-versions

# 権限チェック（スクリプトファイルのみ）
rs_diffcopy -S old -T new -O output -P scripts

# 権限チェック（すべてのファイル）
rs_diffcopy -S old -T new -O output --check-permissions all

# 個別パッチファイルを生成（変更ファイルごとに.patchファイル作成）
rs_diffcopy -S old -T new -O output --patch

# 統合パッチファイルを生成（全変更を1ファイルに）
rs_diffcopy -S old -T new -O output -F changes.patch

# 個別と統合の両方を生成
rs_diffcopy -S old -T new -O output -p -F all.patch

# Excelレポートを出力
rs_diffcopy -S old -T new -O output -E report.xlsx

# Excelレポートを出力（深さ2以上のディレクトリを折りたたみ）
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

# added と deleted 以外すべて表示
rs_diffcopy -S old -T new -O output --filter-status all,^added,^deleted

# 統計情報のみ表示（詳細セクションなし）
rs_diffcopy -S old -T new -O output --stats-only

# File Treeを非表示にして詳細セクションのみ表示
rs_diffcopy -S old -T new -O output --no-tree

# 詳細セクションを非表示にしてFile Treeのみ表示
rs_diffcopy -S old -T new -O output --no-details

# エラーと警告系のみ表示
rs_diffcopy -S old -T new -O output --filter-status error,symlink

# 削除ファイルもコピー（.deleted拡張子付与）
rs_diffcopy -S old -T new -O output --copy-deleted

# タイムスタンプを保持してコピー
rs_diffcopy -S old -T new -O output --preserve-timestamps

# 削除ファイルのコピーとタイムスタンプ保持を組み合わせ
rs_diffcopy -S old -T new -O output --copy-deleted --preserve-timestamps
```

### 5.3 設定ファイル（TOML形式）

※ CLIオプションはkebab-case、設定ファイルはsnake_caseで表記し、名称は1:1に対応する

設定ファイルを使用することで、複雑な設定を再利用可能にします。

#### 設定ファイルの例（diffcopy.toml）

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

# 出力フィルター設定
# filter_status = ["added", "modified"]  # 表示するステータス（省略時は全て表示）
stats_only = false          # 統計情報のみ表示
no_tree = false             # File Treeセクション非表示
no_details = false          # 詳細セクション非表示

# コピーオプション
copy_deleted = false        # 削除ファイルもコピー（.deleted拡張子）
preserve_timestamps = false # タイムスタンプを保持

# パフォーマンス・出力設定
# workers = 4                 # 並列処理のワーカー数（省略時: CPUコア数）
# temp_dir = "/tmp"           # 一時ファイルの保存先（省略時: システムtempディレクトリ）
# color = "auto"              # カラー出力（auto/always/never）
# log_level = "warn"          # ログ出力レベル（error/warn/info/debug）

# 除外パターン（複数指定可）
exclude = [
    "*.log",
    "*.tmp",
    "node_modules/**",
    ".git/**",
    "__pycache__/**"
]
```

#### 設定ファイルの項目

| 項目 | 型 | 必須 | デフォルト | 説明 |
|------|------|:----:|------------|------|
| `source` | string | ✅ | - | 比較元ディレクトリ |
| `target` | string | ✅ | - | 比較先ディレクトリ |
| `output` | string | ✅ | - | 出力先ディレクトリ |
| `exclude` | array | - | `[]` | 除外パターンのリスト |
| `force` | bool | - | `false` | 出力先を削除して再実行 |
| `verbose` | bool | - | `false` | 詳細出力モード |
| `dry_run` | bool | - | `false` | ドライラン |
| `both_versions` | bool | - | `false` | 新旧両方をコピー |
| `summary` | string | - | - | サマリー出力先ファイル |
| `check_permissions` | string | - | `"none"` | 権限チェックモード |
| `patch` | bool | - | `false` | 個別パッチファイル生成 |
| `patch_file` | string | - | - | 統合パッチファイルパス |
| `excel` | string | - | - | Excelレポート出力パス |
| `excel_fold_level` | integer | - | - | Excelファイルツリーの折りたたみレベル（指定深さ以上を折りたたみ） |
| `show_unchanged` | bool | - | `false` | 変更がないファイルをサマリー詳細に表示 |
| `filter_status` | array | - | `[]` | 表示するステータスのリスト（空は全て表示） |
| `stats_only` | bool | - | `false` | 統計情報のみ表示 |
| `no_tree` | bool | - | `false` | File Treeセクションを非表示 |
| `no_details` | bool | - | `false` | 詳細セクションを非表示 |
| `copy_deleted` | bool | - | `false` | 削除ファイルもコピー（.deleted拡張子） |
| `preserve_timestamps` | bool | - | `false` | コピー時にタイムスタンプを保持 |
| `workers` | integer | - | CPUコア数 | 並列処理のワーカー数 |
| `temp_dir` | string | - | システムtempディレクトリ | 一時ファイルの保存先 |
| `color` | string | - | `"auto"` | カラー出力設定 |
| `log_level` | string | - | `"warn"` | ログ出力レベル |
| `three_way` | bool | - | `false` | 三者間比較モード |
| `base` | string | ※ | - | 共通祖先ディレクトリ（三者間モード時必須） |
| `merge_style` | string | - | `"all"` | コンフリクト時のコピー方式 |
| `conflict_only` | bool | - | `false` | コンフリクトのみ出力 |

#### 優先順位

コマンドライン引数と設定ファイルの両方が指定された場合、**コマンドライン引数が優先**されます。

#### 設定ファイルの保存（--save-config）

`-C, --save-config <PATH>` オプションを使用すると、現在のコマンドラインオプションを設定ファイルとして保存できます。

**動作仕様：**
- 差分処理を実行した後、設定ファイルを保存
- 保存される設定ファイルには日本語コメントが付与される
- `dry_run` オプションは常にコメントアウトされた状態で保存（誤って有効にならないよう配慮）
- 未指定のオプションはコメントアウトされたサンプルとして記載

**生成される設定ファイルの例：**

```toml
# rs_diffcopy 設定ファイル
# このファイルは --save-config オプションにより自動生成されました
# 設定を変更して再利用することができます

# 必須設定
source = "./old_version"  # 比較元ディレクトリ（変更する場合はパスを修正してください）
target = "./new_version"  # 比較先ディレクトリ（変更する場合はパスを修正してください）
output = "./diff_output"  # 出力ディレクトリ（変更する場合はパスを修正してください）

# オプション設定
force = false
verbose = false
# 注: dry_run はこの設定ファイルでは無効になっています
# 必要に応じてコメントを外してください
# dry_run = false
both_versions = false
# summary = "./summary.txt"  # サマリー出力ファイル
check_permissions = "none"  # none / scripts / all
patch = false  # 個別パッチファイル生成
# patch_file = ""  # 統合パッチファイル（空欄で無効）
# excel = ""  # Excel出力ファイル（空欄で無効）
# excel_fold_level = 2  # Excelファイルツリーの折りたたみレベル（省略時は折りたたみなし）
show_unchanged = false  # 変更なしファイルをサマリーに表示

# 三者間比較オプション
three_way = false  # 三者間比較モード
# base = "./base_version"  # 共通祖先ディレクトリ
merge_style = "all"  # all / ours / theirs
conflict_only = false  # コンフリクトのみ出力

# 出力フィルター設定
# filter_status = ["added", "modified"]  # 表示するステータス（省略時は全て表示）
stats_only = false  # 統計情報のみ表示
no_tree = false  # File Treeセクション非表示
no_details = false  # 詳細セクション非表示

# コピーオプション
copy_deleted = false  # 削除ファイルもコピー
preserve_timestamps = false  # タイムスタンプを保持

# 除外パターン（glob形式、複数指定可）
# exclude = [
#     "*.log",
#     "*.tmp",
#     "node_modules",
#     ".git",
# ]
```

### 5.4 除外パターン

#### パターン形式

除外パターンはglob形式で指定します。

| パターン | マッチ対象 |
|----------|-----------|
| `*.log` | 全ての `.log` ファイル |
| `node_modules` | `node_modules` ディレクトリとその中身 |
| `__pycache__` | `__pycache__` ディレクトリとその中身 |
| `.git/**` | `.git` ディレクトリ配下全て |
| `build/*.tmp` | `build` 直下の `.tmp` ファイル |

#### パスコンポーネントマッチング

除外パターンは**パス全体**だけでなく、**パスの各コンポーネント（ディレクトリ名・ファイル名）**にもマッチします。

```bash
# "__pycache__" パターンは以下全てにマッチ
rs_diffcopy -S old -T new -O output -e "__pycache__"

# マッチするパス:
#   __pycache__
#   src/__pycache__
#   src/__pycache__/module.pyc
#   lib/utils/__pycache__/helper.pyc
```

| パターン | マッチするパス例 |
|----------|-----------------|
| `__pycache__` | `__pycache__`, `src/__pycache__`, `src/__pycache__/file.pyc` |
| `node_modules` | `node_modules`, `project/node_modules/pkg` |
| `.git` | `.git`, `.git/config`, `submodule/.git/HEAD` |
| `*.log` | `debug.log`, `logs/app.log` |

---

## 6. 差異判定

### 6.1 ファイル内容比較

- **BLAKE3ハッシュ**によるファイル内容比較
  - 高速かつ衝突耐性が極めて高い（256bit出力）
  - SIMD最適化により高いパフォーマンス
- **最適化**: サイズが異なれば即座に「差異あり」と判定（ハッシュ計算を省略）
- サイズが同じ場合のみハッシュを計算して比較

### 6.2 権限・属性チェック

`-P/--check-permissions` オプションで、ファイルの権限・属性の変更を検出できます。

検出された権限変更は `Permission Changes` セクションに詳細が表示されます。また、権限のみが変更されたファイルは、File Treeに `[permission]` タグ付きで表示されます。

#### チェックモード

| モード | 説明 |
|--------|------|
| `none` | チェックしない（デフォルト） |
| `scripts` | スクリプトファイルのみチェック |
| `all` | すべてのファイルをチェック |

#### scriptsモードの対象拡張子

| カテゴリ | 拡張子 |
|----------|--------|
| シェルスクリプト | `.sh`, `.bash`, `.zsh`, `.ksh`, `.fish` |
| Perl | `.pl`, `.pm` |
| Python | `.py`, `.pyw` |
| Ruby | `.rb` |
| JavaScript/Node.js | `.js`, `.mjs` |
| TypeScript | `.ts` |
| PHP | `.php` |
| Lua | `.lua` |
| PowerShell | `.ps1`, `.psm1` |
| Windowsバッチ | `.bat`, `.cmd` |
| 実行ファイル | `.exe`, `.com` |

#### 検出される権限情報

| OS | 検出内容 | 表示例 |
|----|----------|--------|
| Linux/macOS | ファイルモード | `755 -> 644` |
| Windows | 読み取り専用属性 | `writable -> readonly` |

---

## 7. ファイル状態別の扱い

### 7.1 通常モード（デフォルト）

| 状態 | 扱い |
|------|------|
| 新規ファイル | 出力先にコピー |
| 変更ファイル | 出力先にコピー（target側のファイル） |
| 新規ディレクトリ（空含む） | 出力先に作成 |
| 削除ファイル/ディレクトリ | サマリーに記載のみ（コピーしない） |
| シンボリックリンク | サマリーに記載のみ（コピーしない） |
| 特殊ファイル | スキップしてサマリーに記載（ソケット、FIFO、デバイスファイル等） |
| 権限エラー | スキップしてサマリーに記載 |
| 権限変更のみ | 出力先にコピー（target側のファイル）、サマリーのFile Treeに `[permission]` タグで表示 |
| コピー失敗 | スキップしてサマリーに記載、処理は継続 |

### 7.2 --copy-deleted モード

| 状態 | 扱い |
|------|------|
| 削除ファイル | 出力先にコピー（`.deleted`拡張子付与、例: `file.txt` → `file.txt.deleted`） |
| その他 | 通常モードと同じ |

### 7.3 --preserve-timestamps モード

| 状態 | 扱い |
|------|------|
| コピー対象ファイル | ファイルの更新日時（mtime）を保持してコピー |
| その他 | 通常モードと同じ |

### 7.4 --both-versions モード

| 状態 | 扱い |
|------|------|
| 新規ファイル | 出力先にコピー（拡張子なし） |
| 変更ファイル | 新旧両方をコピー（`filename.ext.old` / `filename.ext.new`） |
| その他 | 通常モードと同じ |

※ `--both-versions` と `--copy-deleted` を同時指定した場合、削除ファイルは `.deleted` を優先して1つだけコピーする

### 7.5 特殊ファイル（Unix）

Unix系OSでは、通常のファイルやディレクトリ以外に特殊ファイルが存在します。これらはコピーできないため、自動的にスキップされサマリーに記載されます。

| 種類 | 説明 | 例 |
|------|------|-----|
| Socket | プロセス間通信用ソケット | `pseudo.socket`, `/var/run/*.sock` |
| FIFO (Named Pipe) | 名前付きパイプ | パイプラインで使用 |
| Block Device | ブロックデバイス | `/dev/sda` |
| Char Device | キャラクタデバイス | `/dev/tty` |

**注意**: Windowsでは特殊ファイルは存在しないため、このスキップ処理は発生しません。

---

## 8. 出力構造

### 8.1 通常モード

```
output_dir/
├── src/
│   ├── main.rs          # 変更ファイル
│   └── new_feature.rs   # 新規ファイル
├── docs/                # 新規ディレクトリ（空でも作成）
└── config.toml          # 変更ファイル
```

### 8.2 --copy-deleted モード

```
output_dir/
├── src/
│   ├── main.rs              # 変更ファイル
│   ├── new_feature.rs       # 新規ファイル
│   └── old_module.rs.deleted  # 削除ファイル（source側、.deleted拡張子付与）
├── docs/
└── config.toml
```

### 8.3 --both-versions モード

```
output_dir/
├── src/
│   ├── main.rs.old      # 変更前のファイル（source側）
│   ├── main.rs.new      # 変更後のファイル（target側）
│   └── new_feature.rs   # 新規ファイルはそのまま
├── docs/
├── config.toml.old
└── config.toml.new
```

### 8.4 --patch モード

```
output_dir/
├── src/
│   ├── main.rs          # 変更ファイル
│   ├── main.rs.patch    # 個別パッチファイル
│   └── new_feature.rs   # 新規ファイル（パッチなし）
├── docs/
├── config.toml
└── config.toml.patch
```

### 8.5 パッチ生成

#### パッチ形式

生成されるパッチは**unified diff形式**で、`git apply`コマンドで適用可能です。

```diff
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,5 +1,5 @@
 fn main() {
-    println!("Hello");
+    println!("Hello, World!");
 }
```

#### パッチの適用例

```bash
# 個別パッチの適用
cd target_directory
git apply path/to/file.patch

# 統合パッチの適用
git apply changes.patch
```

#### バイナリファイル

バイナリファイルはパッチ生成をスキップし、サマリーに `[skip]` として記載されます。

バイナリ判定基準：
- ファイル先頭8192バイトにNULLバイト（0x00）が含まれる
- UTF-8としてデコードできない

---

## 9. 進捗表示

処理はフェーズ別に進捗表示されます：

```
[1/5] Scanning Directories...
Found 1234 items.
[2/5] Comparing Files: [=============>              ] 45% (555/1234)
Compared 1234 items.
[3/5] Copying Files...
Copied 100 files.
[4/5] Generating Patches...
Generated 50 patches.
[5/5] Writing Summary...
Completed.
```

### 9.1 処理フェーズ

| フェーズ | 処理内容 | 並列化 |
|---------|---------|:------:|
| Phase 1 | Scanning (ディレクトリ走査) | - |
| Phase 2 | Comparing (ハッシュ比較) | 並列 |
| Phase 3 | Copying (ファイルコピー) | 並列 |
| Phase 4 | Generating patches (パッチ生成) | 並列 |
| Phase 5 | Writing summary | - |

※ Phase 4 は `--patch` または `--patch-file` 指定時のみ表示

### 9.2 表示仕様

- 各フェーズ開始時に `[n/5]` 形式でフェーズ番号を表示
- 比較・コピー処理中はプログレスバーで進捗を表示
- パイプやリダイレクト時はプログレスバーを非表示
- `--verbose` モードでは追加の詳細情報を表示

---

## 10. サマリー出力形式

### 10.1 差分ありの場合

```
rs_diffcopy Summary
===================
Source: /path/to/source
Target: /path/to/target
Output: /path/to/output
Date: 2025-12-18 10:30:00

Options:
  Mode: Dry run (no files copied)
  Copy mode: Both versions (.old/.new)
  Copy deleted: Yes (.deleted)
  Preserve timestamps: Yes
  Permission check: scripts
  Patch mode: Individual files (.patch)
  Combined patch file: changes.patch
  Config file: diffcopy.toml
  Exclude patterns:
    - *.log
    - __pycache__
    - node_modules

Added:         5 files, 1 dir
Modified:      8 files
Deleted:       2 files, 1 dir
Symlinks:      3 files
Special Files: 1 file
Permissions:   2 files
Errors:        1 file
Unchanged:     50 files
--------------------------
Total:        71 items

================
File Tree
================
.
├── src/
│   ├── main.rs [modified]
│   ├── new_feature.rs [added]
│   └── old_module.rs [deleted]
├── docs/ [added]
├── scripts/
│   └── build.sh [permission]
├── config.toml [modified]
├── cache -> /tmp/cache [symlink: added]
├── broken_link -> /nonexistent [symlink: added, broken]
├── old_link -> /old/path [symlink: deleted]
├── changed_link -> /new/path [symlink: changed]
├── became_broken -> /path [symlink: broken]
├── pseudo.socket [special: socket]
├── secret.key [permission denied]
└── legacy/ [deleted]

※ ファイルの内容も同時に変更された場合の権限変更の詳細は `Permission Changes` セクションに一覧表示されます。

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
Directories:
  legacy/

Files:
  src/old_module.rs

================
Symlink Details
================
Added:
  cache -> /tmp/cache
    Type: directory | Status: OK

  broken_link -> /nonexistent
    Type: file | Status: BROKEN (target does not exist)

Deleted:
  old_link -> /old/path
    Type: file

Changed:
  changed_link
    Before: /old/path (file, OK)
    After:  /new/path (file, OK)

  became_broken
    Before: /path (file, OK)
    After:  /path (file, BROKEN)

================
Permission Changes
================
scripts/build.sh: 755 -> 644
src/main.py: 755 -> 644

================
Errors
================
secret.key: Permission denied

================
Special Files (skipped)
================
pseudo.socket (socket)

================
Patch Details
================
Generated: 8 patches, Skipped: 2 (binary), Failed: 1

Generated:
  src/main.rs.patch
  config.toml.patch

Skipped (binary):
  images/logo.png [skip]
  data/db.bin [skip]

Failed:
  corrupted.txt: Failed to read source file

================
Copy Failed
================
Failed: 1 files (Copied: 10 files)

  large_file.dat: No space left on device
```

### 10.2 差分なしの場合

```
rs_diffcopy Summary
===================
Source: /path/to/source
Target: /path/to/target
Output: /path/to/output
Date: 2025-12-18 10:30:00

No differences found.
```

### 10.3 サマリーセクション一覧

| セクション | 表示条件 | 内容 |
|------------|----------|------|
| ヘッダー | 常に表示 | Source, Target, Output, Date |
| Options | オプション指定時 | 使用したオプションの一覧 |
| 統計情報 | 常に表示 | Added, Modified, Deleted, Unchanged等の件数とTotal |
| File Tree | 差分あり時 | ツリー形式の差分一覧 |
| Added Files | 追加あり時 | 追加されたファイル/ディレクトリ一覧 |
| Modified Files | 変更あり時 | 変更されたファイル一覧 |
| Deleted Files | 削除あり時 | 削除されたファイル/ディレクトリ一覧 |
| Unchanged Files | `--show-unchanged`指定時 | 変更がないファイル一覧 |
| Symlink Details | 追加/削除/変更されたシンボリックリンクあり時 | シンボリックリンクの詳細情報（※変更なしは表示されない） |
| Permission Changes | 権限変更あり時 | 権限変更の詳細 |
| Errors | エラーあり時 | 権限エラー詳細 |
| Special Files (skipped) | 特殊ファイルあり時 | スキップされた特殊ファイル一覧（Unix） |
| Patch Details | パッチ生成時 | 生成/スキップ/失敗したパッチ一覧 |
| Copy Failed | コピー失敗時 | コピーに失敗したファイル一覧とエラー理由 |

### 10.3.1 出力先別の形式

二者間比較と三者間比較で、コンソール出力とファイル出力の形式が異なります。

| モード | コンソール出力 | ファイル出力（-s指定時） |
|--------|---------------|------------------------|
| 二者間比較 (File Tree) | ツリー形式 | ツリー形式 |
| 三者間比較 (File Tree) | ツリー形式（コンパクト） | ツリー形式（整列） |

#### 二者間比較のFile Tree形式

**コンソール出力**: コンパクト形式（パス直後にステータス表示）

```
================
File Tree
================
.
├── file1.txt [modified]
├── subdir/
│   ├── added.txt [added]
│   └── nested/
│       └── deep.txt [modified]
└── config.toml [deleted]
```

**ファイル出力**: 整列形式（最長パス幅に合わせてステータス位置を揃える）

```
================
File Tree
================
.
├── file1.txt                        [modified]
├── subdir/
│   ├── added.txt                    [added]
│   └── nested/
│       └── deep.txt                 [modified]
└── 日本語ファイル.txt               [modified]
```

- ファイル出力では、すべてのファイルのステータス表示位置を最長パス幅に合わせて揃える
- 日本語文字は表示幅2、半角文字は表示幅1として計算（`unicode_width`クレートを使用）
- 半角カナ等のUnicode文字も正確に表示幅を計算
- **Box Drawing文字（U+2500-U+257F: │, └, ├, ─等）は表示幅2として計算**（CJK端末互換性のため）

#### 三者間比較のFile Tree形式

三者間比較でもツリー形式を使用し、[Base|Ours|Theirs]のインジケータを付加します。

**コンソール出力**: コンパクト形式（`[○M=]`のように詰めて表示）

```
================
File Tree
================
Legend: [Base|Ours|Theirs] ○=exists -=missing ==same M=modified A=added D=deleted
.
├── file1.txt [○M=] ours-only
├── new_ours.txt [-A-] added-ours
├── subdir/
│   └── nested.txt [○=M] theirs-only
└── 日本語ファイル.txt [○M=] ours-only
```

**ファイル出力**: 整列形式（最長パス幅に合わせてインジケータ位置を揃える）

```
================
File Tree
================
Legend: [Base|Ours|Theirs] ○=exists -=missing ==same M=modified A=added D=deleted
                                     B  O  T
.
├── file1.txt                      [○  M  =] ours-only
├── new_ours.txt                   [-  A  -] added-ours
├── subdir/
│   └── nested.txt                [○  =  M] theirs-only
└── 日本語ファイル.txt             [○  M  =] ours-only
```

- ファイル出力では、すべてのファイルのインジケータ位置を最長パス幅に合わせて揃える
- 日本語文字は表示幅2、半角文字は表示幅1として計算（`unicode_width`クレートを使用）
- 半角カナ等のUnicode文字も正確に表示幅を計算
- **Box Drawing文字（U+2500-U+257F: │, └, ├, ─等）は表示幅2として計算**（CJK端末互換性のため）

### 10.4 統計情報の計算

| 項目 | 説明 |
|------|------|
| Added | targetにのみ存在するファイル/ディレクトリ数 |
| Modified | 両方に存在し、内容が変更されたファイル数 |
| Deleted | sourceにのみ存在するファイル/ディレクトリ数 |
| Symlinks | 追加/削除/変更されたシンボリックリンク数（※変更なしのシンボリックリンクは含まない） |
| Special Files | スキップされた特殊ファイル数（Unix: ソケット、FIFO等） |
| Permissions | 権限のみ変更されたファイル数 |
| Errors | 権限エラー等でスキップされたファイル数 |
| Unchanged | 両方に存在し、変更がないファイル数（**常に統計に表示**、`--show-unchanged`はFile Treeと詳細セクションの表示のみ制御） |
| **Total** | 検査した全ユニークパス数（source ∪ target）

**注意**:
- Totalは `source側のファイル数 + target側のファイル数 - 両方に存在するファイル数` で計算されます
- Unchangedは `--show-unchanged` オプションの有無に関わらず、常に統計情報に正確な数が表示されます
- `--show-unchanged` オプションはFile Treeや詳細セクションへの表示のみを制御し、統計情報には影響しません

### 10.5 ステータスタグ一覧

| タグ | 意味 |
|------|------|
| `[added]` | 新規追加 |
| `[modified]` | 内容変更あり |
| `[deleted]` | 削除（コピーされない） |
| `[unchanged]` | 変更なし（`--show-unchanged`時に表示） |
| `[symlink: added]` | 新規追加されたシンボリックリンク |
| `[symlink: added, broken]` | 新規追加された壊れたシンボリックリンク |
| `[symlink: deleted]` | 削除されたシンボリックリンク |
| `[symlink: changed]` | リンク先が変更されたシンボリックリンク |
| `[symlink: changed, broken]` | リンク先が変更され、かつ壊れたシンボリックリンク |
| `[symlink: broken]` | 同じリンク先で壊れた状態になったシンボリックリンク |
| `[special: socket]` | Unixソケットファイル（スキップ） |
| `[special: fifo]` | FIFO/名前付きパイプ（スキップ） |
| `[special: block device]` | ブロックデバイス（スキップ） |
| `[special: char device]` | キャラクタデバイス（スキップ） |
| `[permission]` | 権限のみ変更 |
| `[permission denied]` | 権限エラー（スキップ） |
| `[skip]` | パッチ生成スキップ（バイナリファイル） |

※ シンボリックリンクはコピーされず、Symlink Detailsセクションに詳細が表示される（変更なしのシンボリックリンクは統計にもFile Treeにも含まれない）
※ バイナリファイルはパッチ生成時にスキップされ、Patch Detailsセクションに記載
※ 特殊ファイルはUnix系OSでのみ検出され、スキップされてSpecial Files (skipped)セクションに記載

### 10.6 Excelレポート形式

`--excel <PATH>` オプションを使用すると、サマリーをExcelファイル(.xlsx)として出力できます。

#### シート構成

| シート名 | 内容 |
|----------|------|
| Summary | 基本情報、オプション、統計情報 |
| File Tree | ツリー形式のファイル一覧（セルでインデント表示、折りたたみ対応） |
| Details | 追加/変更/削除ファイル、シンボリックリンク、権限変更、パッチの詳細一覧 |

#### 共通書式設定

- **罫線**: 全てのデータ領域には外枠罫線（細線）を設定し、視認性を向上
- **タイトル**: 青色太字、16pt、左揃え
- **セクションヘッダー**: 青背景（#4472C4）、白文字、太字、12pt
- **ラベル列**: 太字で表示（`Source:`、`Target:`等）
- **ステータス色分け**:
  - 追加（Added）: 緑色 (#008000)
  - 変更（Modified）: 青色 (#0066CC)
  - 削除（Deleted）: 赤色 (#CC0000)
  - シンボリックリンク: 紫色 (#9933FF)
  - 変更なし（Unchanged）: グレー (#808080)

#### Summaryシート詳細

| セクション | 内容 | 書式 |
|-----------|------|------|
| タイトル | "rs_diffcopy Summary" | 16pt、青色太字 |
| 基本情報 | Source, Target, Output, Date | ラベル列は太字 |
| Options | 使用したオプション一覧（dry_run, both_versions, copy_deleted, preserve_timestamps, check_permissions, patch, patch_file, show_unchanged, filter_status, exclude, stats_only, no_tree, no_details） | ラベル列は太字、セクションヘッダーは青背景 |
| Statistics | 統計情報（Added, Modified, Deleted等） | セクションヘッダーは青背景、ヘッダーの幅は2列に適用 |

#### File Treeシート詳細

- **ツリー構造**: 各パスコンポーネント（ディレクトリ階層）をセル単位で分離して表示
  - 各ディレクトリ階層を別々の列に配置し、ファイル名は最後の列に配置
  - **重複省略**: 上のセルと同じ値の場合は記載しない（ツリー構造を視覚的に表現）
  - **パス展開**: 新しいディレクトリに初めて入る場合、中間ディレクトリを各行に展開する
    - これにより`--excel-fold-level`が深いパスでも正しく動作する
    - 例: `a/b/c/d.txt`が最初のパスの場合:
      ```
         | A | B | C | D    | Status |
       1 |a/ |   |   |      |        |
       2 |   |b/ |   |      |        |
       3 |   |   |c/ |      |        |
       4 |   |   |   | d.txt| added  |
      ```
    - 中間ディレクトリ行にはStatusが空（ファイルではないため）
  - ディレクトリは末尾に"/"を付与
  - 例: 以下のファイル構造の場合
    ```
    ├── a.txt
    ├── b/
    │   ├── d.txt
    │   └── e/
    │       └── g.txt
    └── c/
        ├── e.txt
        └── f/
            └── h.txt
    ```
    Excelセル表示:
    ```
       |   A   |  B   |   C   | Status  |
     1 | a.txt |      |       | added   |
     2 | b/    |      |       |         |
     3 |       | d.txt|       | modified|
     4 |       | e/   |       |         |
     5 |       |      | g.txt | modified|
     6 | c/    |      |       |         |
     7 |       | e.txt|       | deleted |
     8 |       | f/   |       |         |
     9 |       |      | h.txt | modified|
    ```
  - A列の値は行2で"b/"、行6で"c/"と変わるタイミングのみ記載
  - B列の値は親ディレクトリが変わるか、値自体が変わる場合に記載
  - 中間ディレクトリ行（Statusが空の行）はfold-levelの対象になる
- **フォント**: 等幅フォント（Consolas）を使用
- **ディレクトリ区切り罫線**: 第1階層（A列）のディレクトリが変わるタイミングで下罫線を追加
  - 上記例では行1、行5、行9の下に罫線を追加し、ディレクトリ単位を視覚的に区切る
- **行グループ化（折りたたみ）**: `-L, --excel-fold-level <LEVEL>` オプションで指定した深さ以上の項目をグループ化
  - 深さNは、ルートからのパス階層数（ルート直下=深さ1）
  - グループ化は各ディレクトリ単位で行う（連続した行ではなく、同一ディレクトリ配下をまとめる）
  - 例: `-L 2` の場合
    - 行3-5（b/配下）と行7-9（c/配下）がそれぞれ別グループとして折りたたみ可能
  - 例: `-L 3` の場合
    - 行5のみ（e/配下）と行9のみ（f/配下）がそれぞれ別グループとして折りたたみ可能
  - Excelの行グループ化機能（`group_rows`）を使用
- **ヘッダー行**: 動的列数（最大深さに応じて拡張）、最終列がStatus、青背景

#### Options表示仕様

SummaryファイルおよびExcelのOptionsセクションにおける`filter_status`の表示:

- `--filter-status added,modified` → "Filter status: added, modified"
- `--filter-status all` → "Filter status: all"
- `--filter-status all,^deleted` → "Filter status: all, ^deleted"
- `--filter-status ^deleted,^unchanged` → "Filter status: all (implied), ^deleted, ^unchanged"

※ 除外指定（^prefix）のみの場合は、暗黙的に"all"が適用される

#### Detailsシート詳細

- **ヘッダー行**: Status, Directory, File, Details の4列
  - ヘッダーの背景色幅は全4列に適用（merge_rangeではなく個別セル設定）
- **セクション分離**: ステータス別にセクションを分け、セクションヘッダー行を挿入
- **パス分離**: フルパスを「Directory」列と「File」列に分離して表示

### 10.7 出力フィルター機能

出力結果（コンソール、サマリーファイル、Excelレポート）とコピー対象に対して、表示内容をフィルタリングできます。

#### ステータスフィルター（--filter-status）

指定したステータスのファイルのみをコピー/表示します。

| ステータス値 | 対象 |
|-------------|------|
| `all` | 全ステータス（除外指定と組み合わせて使用） |
| `added` | 新規追加されたファイル/ディレクトリ |
| `modified` | 内容が変更されたファイル |
| `deleted` | 削除されたファイル/ディレクトリ |
| `unchanged` | 変更がないファイル |
| `symlink` | シンボリックリンク（追加/削除/変更） |
| `special` | 特殊ファイル（ソケット、FIFO等） |
| `error` | 権限エラー等でスキップされたファイル |
| `permission` | 権限のみ変更されたファイル |

※ `unchanged` を詳細セクションやFile Treeに表示するには `--show-unchanged` が必要

**除外指定（^プレフィックス）：**

ステータス値の先頭に`^`を付けると、そのステータスを表示対象から除外します。

```bash
# unchanged以外すべて表示
--filter-status all,^unchanged

# added と modified 以外すべて表示
--filter-status all,^added,^modified

# addedとmodifiedを追加し、addedを除外（結果: modifiedのみ）
--filter-status added,modified,^added
```

**三者間モード用ステータス値：**

| ステータス値 | 対象 |
|-------------|------|
| `all` | 全ステータス（除外指定と組み合わせて使用） |
| `unchanged` | 3つとも同一（変更なし） |
| `ours-only` | oursのみ変更 |
| `theirs-only` | theirsのみ変更 |
| `both-same` | 両方が同じ変更 |
| `conflict` | コンフリクト（両方が異なる変更） |
| `added-ours` | oursでのみ追加 |
| `added-theirs` | theirsでのみ追加 |
| `added-both-same` | 両方で追加（同一内容） |
| `added-both-diff` | 両方で追加（異なる内容）- コンフリクト |
| `deleted-ours` | oursで削除 |
| `deleted-theirs` | theirsで削除 |
| `deleted-both` | 両方で削除 |
| `modify-delete` | oursで変更、theirsで削除 - コンフリクト |
| `delete-modify` | oursで削除、theirsで変更 - コンフリクト |

**三者間モード用グループキーワード：**

複数のステータスをまとめて指定できるグループキーワード：

| グループ | 含まれるステータス |
|----------|-------------------|
| `added` | added-ours, added-theirs, added-both-same, added-both-diff |
| `modified` | ours-only, theirs-only, both-same, conflict |
| `deleted` | deleted-ours, deleted-theirs, deleted-both |
| `conflicts` | conflict, added-both-diff, modify-delete, delete-modify |

**グループキーワードの展開：**

グループキーワードは指定時（追加・除外どちらも）に自動的に含まれるステータスに展開されます：

```bash
# 追加系のみ表示
--filter-status added
# → added-ours, added-theirs, added-both-same, added-both-diff に展開

# 追加系を除外（全ステータスから追加系を除く）
--filter-status all,^added
# → all, ^added-ours, ^added-theirs, ^added-both-same, ^added-both-diff と同義

# 削除系を除外（暗黙のall + 削除系除外）
--filter-status ^deleted
# → ^deleted-ours, ^deleted-theirs, ^deleted-both に展開

# 追加と変更のみ表示（削除と変更なしを除外）
--filter-status added,modified
```

**動作仕様：**

- 指定は左から右へ順番に処理（後勝ち）
- `all` を指定すると全ステータスを対象に追加
- `^`プレフィックス付きは対象から除外
- **最初のフィルターが除外（^）の場合、全ステータスから開始**（例: `^deleted`は「deleted以外すべて」と同義）
- プレフィックスなしは対象に追加
- 統計情報は**フィルター前の全体数**を表示
- フィルター適用時は統計情報に「(filtered out)」を表示
- File Tree、詳細セクション、Excelレポートにはフィルター後のファイルのみ表示
- コピー対象はフィルター後のファイルのみ（`--dry-run` 時はコピーされない）

**処理例：**

```
# 例1: all,^unchanged
1. all → {added, modified, deleted, unchanged, symlink, special, permission, error}
2. ^unchanged → unchangedを除外
→ 結果: {added, modified, deleted, symlink, special, permission, error}

# 例2: added,modified,^added
1. added → {added}
2. modified → {added, modified}
3. ^added → addedを除外
→ 結果: {modified}

# 例3: all,^unchanged,unchanged
1. all → 全ステータス
2. ^unchanged → unchangedを除外
3. unchanged → unchangedを追加
→ 結果: 全ステータス（後勝ち）
```

#### セクションフィルター

| オプション | 効果 |
|-----------|------|
| `--stats-only` | ヘッダー、Options、統計情報のみ表示（File Tree、詳細セクションを非表示） |
| `--no-tree` | File Treeセクションを非表示 |
| `--no-details` | Added/Modified/Deleted Files等の詳細セクションを非表示 |

**優先順位：**

- `--stats-only` は `--no-tree` と `--no-details` を暗黙的に有効化
- `--stats-only` と `--no-tree`/`--no-details` の同時指定は冗長だが許容

#### Excelレポートへの適用

- ステータスフィルターはExcelの全シートに適用
- `--stats-only` はコンソール表示にのみ影響するため、Excel出力には影響しません。`--stats-only` 指定時でも、`--excel` が指定されていれば**完全なレポートがExcelファイルに出力されます**。
- `--no-tree` 指定時はFile Treeシートが空になる
- `--no-details` 指定時はDetailsシートが空になる

#### 出力例（フィルター適用時）

```
rs_diffcopy Summary
===================
Source: /path/to/source
Target: /path/to/target
Output: /path/to/output
Date: 2025-12-18 10:30:00

Options:
  Filter status: added, modified

Added:         5 files, 1 dir
Modified:      8 files
Deleted:       2 files, 1 dir    (filtered out)
Symlinks:      3 files           (filtered out)
...
--------------------------
Total:        71 items
Showing:      13 items (filtered)

================
File Tree (filtered)
================
.
└── src/
    ├── main.rs [modified]
    └── new_feature.rs [added]

================
Added Files (filtered)
================
Files:
  src/new_feature.rs

================
Modified Files (filtered)
================
  src/main.rs
```

---

## 11. 終了コード

| コード | 意味 | 説明 |
|:------:|------|------|
| 0 | 正常終了（差分あり） | 差分が検出され、正常に処理完了 |
| 1 | エラー終了 | ディレクトリが存在しない、権限不足など |
| 2 | 正常終了（差分なし） | 比較対象に差分がなかった |
| 3 | 正常終了（コンフリクトあり） | 三者間モードでコンフリクト検出 |

---

## 12. エラーハンドリング

### 12.1 処理停止するエラー

| ケース | 挙動 |
|--------|------|
| source/targetディレクトリが存在しない | エラー終了（コード1） |
| 出力先が既に存在 | エラー終了（`--force`で全削除して実行可能） |
| 設定ファイルの必須項目不足 | エラー終了（コード1） |
| 無効なglob パターン | エラー終了（コード1） |
| 危険なパスへの`--force` | エラー終了（コード1）、確認なしで即時拒否 |
| `--force`でのユーザー拒否 | エラー終了（コード1） |

### 12.2 処理継続するエラー（サマリーに記載）

以下のエラーは処理を停止せず、サマリーとExcelレポートにエラー内容を記録して処理を継続します。

| ケース | 挙動 | サマリーセクション |
|--------|------|-----------------|
| ファイル読み取り権限エラー | スキップ | Errors |
| シンボリックリンク | スキップ | Symlink Details |
| 特殊ファイル（ソケット、FIFO等） | スキップ | Special Files (skipped) |
| ファイルコピー失敗 | スキップ | Copy Failed |
| パッチ生成失敗 | スキップ | Patch Details (Failed) |

**設計思想**: 大規模なディレクトリ比較では一部のファイルでエラーが発生することがあります。単一のエラーで全処理を停止するのではなく、可能な限り処理を継続し、エラー情報をサマリーに記録することで、ユーザーは処理結果を確認しながら問題のあるファイルを特定できます。

### 12.3 エラーメッセージ一覧

| エラー | メッセージ例 |
|--------|-------------|
| ディレクトリ未検出 | `Error: Source directory '/path/to/source' does not exist` |
| ファイル読み取り失敗 | `Error: Failed to read file '/path/to/file': Permission denied` |
| 出力先が既に存在 | `Error: Output directory '/path/to/output' already exists. Use --force to overwrite.` |
| 設定ファイル読み込み失敗 | `Error: Failed to read config file 'path/to/config.toml': <詳細>` |
| 設定ファイル必須項目不足 | `Error: Missing required field 'source' in config file` |
| 無効な除外パターン | `Error: Invalid glob pattern '**[invalid'` |
| 危険なパスへの--force | `Error: Cannot use --force on protected path '/home'` |

---

## 13. --force オプションの安全機能

`--force`オプションは出力先ディレクトリを削除するため、安全機能を実装しています。

### 13.1 危険なパスの保護

以下のパスは`--force`で指定しても**確認なしで即時エラー終了**します（パスが**完全一致**する場合のみ。配下のサブディレクトリは対象外）：

#### Linux/macOS

| カテゴリ | 保護対象 |
|----------|----------|
| システムディレクトリ | `/`, `/bin`, `/boot`, `/dev`, `/etc`, `/home`, `/lib`, `/lib32`, `/lib64`, `/libx32`, `/media`, `/mnt`, `/opt`, `/proc`, `/root`, `/run`, `/sbin`, `/srv`, `/sys`, `/tmp`, `/usr`, `/var` |
| ユーザーホーム | `/home/<username>` |

#### Windows

| カテゴリ | 保護対象 |
|----------|----------|
| ドライブルート | `C:\`, `D:\` など |
| システムディレクトリ | `C:\Windows`, `C:\Program Files`, `C:\Program Files (x86)`, `C:\ProgramData` |
| ユーザーディレクトリ | `C:\Users`, `C:\Users\<username>`, `C:\Users\<username>\<foldername>` |

**保護理由（Windowsユーザーディレクトリ）**:
`C:\Users\<username>\<foldername>` には AppData などの重要なフォルダーが含まれ、通常はアプリケーションの出力先になりえません。
このため安全のために保護対象としています。

### 13.2 削除確認プロンプト

危険なパス以外の場合、`--force`使用時に確認プロンプトを表示します：

```
Output directory '/path/to/output' already exists.
  Contains: 45 files, 12 directories
  Total size: 1.2 MB

Delete and continue? [y/N]:
```

- `y` を入力すると削除を実行
- それ以外の入力はキャンセル（エラー終了）

※ 安全のため、確認プロンプトをスキップするオプションはありません。

---

## 14. 文字エンコーディング

| 条件 | エンコーディング |
|------|-----------------|
| Windows + コンソール出力 | CP932 (Shift-JIS) |
| Windows + パイプ/リダイレクト | UTF-8 |
| Linux/macOS | UTF-8 |
| サマリーファイル (`-s`) | UTF-8 |
| Excelファイル (`--excel`) | UTF-8 |

### 日本語対応

- 日本語ファイルパスを正しく処理
- Windowsコンソールでは自動的にCP932に変換して文字化けを防止
- パイプやファイルリダイレクト時はUTF-8で出力

---

## 15. 三者間差分機能

### 15.1 概要

三者間差分（Three-way diff）機能は、共通の祖先（base）と2つの派生バージョン（ours/theirs）を比較し、マージ作業を支援します。

**ユースケース**:
- ブランチマージ前のコンフリクト事前確認
- 複数人で並行開発した変更の統合確認
- フォークしたプロジェクトの差分把握

### 15.2 CLI仕様（三者間モード）

```
rs_diffcopy --three-way [OPTIONS]

必須オプション:
  -B, --base <PATH>              共通祖先ディレクトリ
  -S, --source <PATH>            自分の変更（ours）
  -T, --target <PATH>            相手の変更（theirs）
  -O, --output <PATH>            差分ファイルの出力先

三者間モード専用オプション:
  -3, --three-way                三者間比較モードを有効化
  -M, --merge-style <STYLE>      コンフリクト時のコピー方式（all/ours/theirs）
      --conflict-only            コンフリクト候補のみ出力

既存オプション（二者間と共通）:
  -e, --exclude <PATTERN>        除外パターン
  -f, --force                    出力先を全削除して再実行
  -s, --summary <PATH>           サマリーをファイルに出力
  --filter-status <STATUS>       指定ステータスのファイルのみコピー/表示
  --stats-only                   統計情報のみ表示
  --no-tree                      File Treeセクションを非表示
  --no-details                   詳細セクションを非表示
  -v, --verbose                  詳細出力モード
  -n, --dry-run                  ドライラン
  -E, --excel <PATH>             Excelレポート出力
  -C, --save-config <PATH>       設定ファイル保存
```

※ 三者間モードでは `--both-versions` / `--copy-deleted` / `--preserve-timestamps` は適用されない

### 15.3 使用例

```bash
# 基本的な三者間比較
rs_diffcopy --three-way -B base_dir -S my_changes -T their_changes -O output

# コンフリクト候補のみ抽出
rs_diffcopy -3 -B base -S ours -T theirs -O output --conflict-only

# Excelレポート付き
rs_diffcopy -3 -B base -S ours -T theirs -O output -E report.xlsx

# 設定ファイルを使用
rs_diffcopy --config three_way.toml
```

### 15.4 設定ファイル（三者間モード）

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

# 出力フィルター設定
# filter_status = ["conflict", "ours-only"]  # 表示するステータス
stats_only = false
no_tree = false
no_details = false
```

### 15.5 ファイル状態の判定

三者間比較では、各ファイルを以下の状態に分類します：

| 状態 | Base | Ours | Theirs | 説明 |
|------|:----:|:----:|:------:|------|
| `unchanged` | ○ | = | = | 3つとも同一（変更なし） |
| `ours-only` | ○ | ≠ | = | oursのみ変更 |
| `theirs-only` | ○ | = | ≠ | theirsのみ変更 |
| `both-same` | ○ | ≠ | ≠ (同一) | 両方が同じ変更（ours = theirs ≠ base） |
| `conflict` | ○ | ≠ | ≠ (異) | 両方が異なる変更（コンフリクト候補） |
| `added-ours` | - | ○ | - | oursでのみ追加 |
| `added-theirs` | - | - | ○ | theirsでのみ追加 |
| `added-both-same` | - | ○ | ○ (同一) | 両方で同じファイルを追加（ours = theirs） |
| `added-both-diff` | - | ○ | ○ (異) | 両方で異なるファイルを追加（コンフリクト） |
| `deleted-ours` | ○ | - | ○ | oursで削除 |
| `deleted-theirs` | ○ | ○ | - | theirsで削除 |
| `deleted-both` | ○ | - | - | 両方で削除 |
| `modify-delete` | ○ | ≠ | - | oursで変更、theirsで削除（コンフリクト） |
| `delete-modify` | ○ | - | ≠ | oursで削除、theirsで変更（コンフリクト） |

※ `○` = 存在、`-` = 存在しない、`=` = baseと同一、`≠` = baseと異なる

### 15.6 コンフリクト判定

以下の状態は**コンフリクト候補**として特別にマークされます：

| コンフリクト種別 | 状態 | 説明 |
|-----------------|------|------|
| 内容コンフリクト | `conflict` | 同じファイルを異なる内容に変更 |
| 追加コンフリクト | `added-both-diff` | 同名ファイルを異なる内容で追加 |
| 変更/削除コンフリクト | `modify-delete` | 一方が変更、他方が削除 |
| 削除/変更コンフリクト | `delete-modify` | 一方が削除、他方が変更 |

### 15.7 出力構造（三者間モード）

#### 通常モード（--merge-style all）

```
output_dir/
├── src/
│   ├── main.rs                    # ours-only: oursの変更をコピー
│   ├── utils.rs                   # theirs-only: theirsの変更をコピー
│   ├── config.rs                  # both-same: どちらか一方をコピー
│   ├── handler.rs.base            # conflict: baseをコピー
│   ├── handler.rs.ours            # conflict: oursをコピー
│   ├── handler.rs.theirs          # conflict: theirsをコピー
│   └── new_feature.rs             # added-ours: oursをコピー
├── lib/
│   ├── helper.rs.base             # modify-delete: baseをコピー
│   └── helper.rs.ours             # modify-delete: oursをコピー（theirsは削除のため無し）
└── docs/
    └── readme.md                  # added-theirs: theirsをコピー
```

#### --merge-style ours

コンフリクト時はoursを優先してコピー（`.ours`拡張子なし）

#### --merge-style theirs

コンフリクト時はtheirsを優先してコピー（`.theirs`拡張子なし）

### 15.8 サマリー出力形式（三者間モード）

三者間比較では、File Tree形式にステータスインジケータを付加して出力します。
コンソール出力とファイル出力で形式が異なります。

#### コンソール出力（コンパクト形式）

```
rs_diffcopy Summary (Three-way)
===============================
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
...
--------------------------
Total           |   78
Conflicts       |    4

================
File Tree
================
Legend: [Base|Ours|Theirs] ○=exists -=missing ==same M=modified A=added D=deleted
.
├── file1.txt [○M=] ours-only
├── new_ours.txt [-A-] added-ours
├── handler.rs [○MM] CONFLICT
├── subdir/
│   └── nested.txt [○=M] theirs-only
└── 日本語ファイル.txt [○M=] ours-only
```

#### ファイル出力（整列形式）

ファイル出力では、パス幅に応じて整列した形式で出力します。
日本語などの全角文字は2文字分として幅計算されます。

```
================
File Tree
================
Legend: [Base|Ours|Theirs] ○=exists -=missing ==same M=modified A=added D=deleted
                                     B  O  T
.
├── file1.txt                      [○  M  =] ours-only
├── new_ours.txt                   [-  A  -] added-ours
├── handler.rs                     [○  M  M] CONFLICT
├── subdir/
│   └── nested.txt                [○  =  M] theirs-only
└── 日本語ファイル.txt             [○  M  =] ours-only
```

#### インジケータの意味

| インジケータ | 意味 |
|-------------|------|
| `○` | ファイルが存在 |
| `-` | ファイルが存在しない |
| `=` | baseと同一（変更なし） |
| `M` | 変更あり |
| `A` | 新規追加 |
| `D` | 削除 |

#### Conflict Details セクション

```
================
Conflict Details
================
1. src/handler.rs
   Type: Content conflict
   Base:   abc123... (1024 bytes)
   Ours:   def456... (1100 bytes)
   Theirs: 789abc... (1050 bytes)

2. lib/helper.rs
   Type: Modify/Delete conflict
   Ours: Modified (500 bytes)
   Theirs: Deleted
```

### 15.9 Excelレポート（三者間モード）

| シート名 | 内容 |
|----------|------|
| Summary | 基本情報、オプション、統計情報、コンフリクト数 |
| File Tree | ツリー形式のファイル一覧（二者間比較と同じ形式、三者間ステータス表示） |
| Conflicts | コンフリクト候補の詳細一覧 |
| Copied Files | コピーされたファイルの一覧 |

#### Summaryシート詳細

- **基本情報セクション（Base, Ours, Theirs, Output, Date）**: 外枠罫線を適用
- **Options**: オプションセクションには外枠罫線を適用
- **Change Matrix**: 統計情報セクションには外枠罫線を適用
- 三者間モードでも以下のオプションがOptionsセクションに表示される：
  - `--filter-status`: フィルタリングするステータス
  - `--merge-style`: マージスタイル
  - `--conflict-only`: コンフリクトのみ出力
  - `--exclude`: 除外パターン

#### File Treeシート

二者間比較と同じツリー形式を使用：
- 各パスコンポーネントをセル単位で分離表示
- Status列に三者間ステータス（unchanged, ours-only, theirs-only, conflict等）を表示
- `--excel-fold-level`オプションで折りたたみレベルを指定可能
- ディレクトリ区切り罫線、パス展開機能も二者間と同様に動作

#### Conflictsシート詳細

| 列 | 内容 |
|----|------|
| Directory | ディレクトリパス |
| Filename | ファイル名 |
| Type | コンフリクトタイプ |
| Base Hash | Baseのハッシュ（先頭8文字） |
| Ours Hash | Oursのハッシュ（先頭8文字） |
| Theirs Hash | Theirsのハッシュ（先頭8文字） |
| Base Size | Baseのサイズ |
| Ours Size | Oursのサイズ |
| Theirs Size | Theirsのサイズ |

- ヘッダー行の背景色は全列（9列）に適用
- パスは「Directory」と「Filename」に分離して表示
- **データセルには外枠罫線を適用**

#### Copied Filesシート詳細

| 列 | 内容 |
|----|------|
| Directory | ディレクトリパス |
| Filename | ファイル名 |
| Status | ステータス |
| Source | コピー元 |

- ヘッダー行の背景色は全列（4列）に適用
- パスは「Directory」と「Filename」に分離して表示
- **データセルには外枠罫線を適用**

#### ステータスの色分け

| 状態 | 色 |
|------|-----|
| unchanged | グレー (#808080) |
| ours-only | 緑 (#008000) |
| theirs-only | 青 (#0066CC) |
| both-same | 水色 (#00B0F0) |
| conflict | 赤・太字 (#CC0000) |
| added-* | 薄緑 (#92D050) |
| deleted-* | 薄赤 (#FFC7CE) |

### 15.10 終了コード（三者間モード）

| コード | 意味 |
|:------:|------|
| 0 | 正常終了（差分あり、コンフリクトなし） |
| 1 | エラー終了 |
| 2 | 正常終了（差分なし） |
| 3 | 正常終了（コンフリクトあり） |

---

## 16. 将来の機能検討

### 16.1 v1.1 で検討する機能

| 機能 | オプション案 | 説明 |
|------|-------------|------|
| インクルードパターン | `-i, --include <PATTERN>` | 特定のファイルのみを対象にする（excludeの逆） |
| 日付フィルタ | `--newer-than <DATE>` | 指定日以降に変更されたファイルのみ対象 |
| サイズフィルタ | `--min-size`, `--max-size` | 最小/最大サイズでフィルタ |
| JSON出力 | `--json` | 機械可読なJSON形式でサマリー出力 |
| 静音モード | `-q, --quiet` | 最小限の出力のみ（スクリプト向け） |

### 16.2 v1.2 以降で検討する機能

| 機能 | 説明 |
|------|------|
| ハッシュキャッシュ | 前回の比較結果をキャッシュし、増分比較で高速化 |
| HTMLレポート | ブラウザで閲覧可能なレポート生成 |
| ウォッチモード | ディレクトリの変更を監視して自動実行 |

### 16.3 v1.0.0 機能充足度

v1.0.0は想定ユーザー（初心者・非技術者）に対して以下の点で十分な機能を提供：

- **シンプルなCLI**: 最小限のオプションで基本機能を利用可能
- **分かりやすい出力**: ツリー形式のサマリーで変更点を視覚的に把握、カラー出力対応
- **安全な操作**: ドライランモードで事前確認、`--force`なしでは上書きしない、危険なパスの保護
- **再利用性**: TOML設定ファイルで複雑な設定を保存・再利用
- **クロスプラットフォーム**: Windows/Linux/macOSで同一の動作
- **パッチ生成**: `git apply`互換のunified diff形式パッチを生成可能
- **Excelレポート**: 非技術者にも見やすいExcel形式のレポート出力
- **三者間比較**: base/ours/theirsの3ディレクトリを比較してコンフリクト検出
- **削除ファイルのコピー**: source側にのみ存在するファイルを`.deleted`拡張子付きで抽出
- **タイムスタンプ保持**: コピー時にファイルの更新日時を保持
