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
- TOML設定ファイルによる複雑な設定の再利用
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

### 優先順位

コマンドライン引数と設定ファイルの両方が指定された場合、**コマンドライン引数が優先**されます。

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

**注意:** Optionsセクションおよび各詳細セクションは、該当する項目がある場合のみ表示されます。

### ステータスタグ

| タグ | 意味 |
|------|------|
| `[added]` | 新規追加 |
| `[modified]` | 変更あり |
| `[deleted]` | 削除（コピーされない） |
| `[symlink: added]` | 新規追加されたシンボリックリンク |
| `[symlink: added, broken]` | 壊れたシンボリックリンク |
| `[symlink: deleted]` | 削除されたシンボリックリンク |
| `[symlink: changed]` | リンク先が変更されたシンボリックリンク |
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
[1/4] Scanning directories...
Found 1234 items.
[2/4] Comparing: [=============>              ] 45% (555/1234)
Compared 1234 items.
[3/4] Copying files...
Copied 100 files.
[4/4] Writing summary...
Done.
```

**処理フェーズ:**
| フェーズ | 処理内容 | 並列化 |
|---------|---------|:------:|
| Phase 1 | Scanning | - |
| Phase 2 | Comparing | 並列 |
| Phase 3 | Copying | 並列 |
| Phase 4 | Summary | - |

### 終了コード

| コード | 意味 |
|--------|------|
| 0 | 正常終了（差分あり） |
| 1 | エラー終了 |
| 2 | 正常終了（差分なし） |

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
