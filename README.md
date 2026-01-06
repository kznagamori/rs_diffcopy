# rs_diffcopy

[English](README_en.md)

2つのディレクトリを比較し、差分があるファイルのみを抽出するCLIツールです。

## 特徴

- 差分ファイルを階層構造を維持したまま別フォルダへ抽出
- BLAKE3ハッシュによる高速かつ正確なファイル比較
- **並列処理**による高速な比較・コピー（rayon使用）
- フェーズ別進捗表示で処理状況を可視化
- 追加・変更・削除ファイルをツリー形式で表示
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
| `-h, --help` | ヘルプ表示 |
| `-V, --version` | バージョン表示 |

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
| `source` | string | ✅ | 比較元ディレクトリ |
| `target` | string | ✅ | 比較先ディレクトリ |
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
| 削除ファイル/ディレクトリ | サマリーに記載のみ |
| シンボリックリンク | サマリーに記載のみ |
| 特殊ファイル（Unix） | スキップしてサマリーに記載 |
| コピー失敗 | スキップしてサマリーに記載、処理は継続 |

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

## ライセンス

MIT License

## 貢献

バグ報告や機能要望は [Issues](https://github.com/kznagamori/rs_diffcopy/issues) へお願いします。
