# rs_diffcopy

[English](README_en.md)

ディレクトリ間の差分を比較し、変更されたファイルを構造を保持したまま抽出するCLIツールです。
二方向比較と三方向比較（マージ）に対応しています。

## 特徴

- **二方向比較**: ソースとターゲットのディレクトリを比較し、追加・変更・削除されたファイルを検出
- **三方向比較**: ベース、Ours、Theirsの3つのディレクトリを比較し、コンフリクトを検出
- **構造保持**: 出力ディレクトリにオリジナルのディレクトリ構造を保持
- **日本語パス対応**: 日本語を含むファイルパスを正しく処理
- **パッチ生成**: 変更ファイルに対してunified diff形式のパッチを生成
- **Excelレポート**: 比較結果をExcelファイル（.xlsx）として出力
- **並列処理**: 大量のファイルを高速に処理
- **設定ファイル**: TOML形式の設定ファイルで再利用可能

## インストール

### ソースからビルド

```bash
git clone https://github.com/kznagamori/rs_diffcopy.git
cd rs_diffcopy
cargo build --release
```

ビルドされたバイナリは `target/release/rs_diffcopy` に生成されます。

### Cargoでインストール

```bash
cargo install --git https://github.com/kznagamori/rs_diffcopy.git
```

## 基本的な使い方

### 二方向比較

```bash
# 基本的な使い方
rs_diffcopy -S <ソース> -T <ターゲット> -O <出力先>

# 例: oldディレクトリとnewディレクトリを比較し、diffディレクトリに出力
rs_diffcopy -S ./old -T ./new -O ./diff
```

### 三方向比較

```bash
# 三方向比較（マージ）
rs_diffcopy --three-way -B <ベース> -S <Ours> -T <Theirs> -O <出力先>

# 例: 共通の祖先からの変更を比較
rs_diffcopy --three-way -B ./base -S ./ours -T ./theirs -O ./merged
```

## 全オプション

| オプション | 短縮形 | 説明 |
|-----------|--------|------|
| `--source` | `-S` | ソースディレクトリ |
| `--target` | `-T` | ターゲットディレクトリ |
| `--output` | `-O` | 出力ディレクトリ |
| `--config` | `-c` | 設定ファイルのパス |
| `--exclude` | `-e` | 除外パターン（glob形式、複数指定可） |
| `--force` | `-f` | 出力ディレクトリを強制上書き |
| `--summary` | `-s` | サマリーをファイルに保存 |
| `--verbose` | `-v` | 詳細出力モード（処理中ファイル名を表示） |
| `--dry-run` | `-n` | ドライラン（実際にコピーしない） |
| `--both-versions` | `-b` | 新旧両バージョンをコピー（.old/.new） |
| `--check-permissions <MODE>` | `-P` | 権限変更チェック（none/scripts/all） |
| `--patch` | `-p` | 個別パッチファイルを生成 |
| `--patch-file` | `-F` | 統合パッチファイルを生成 |
| `--excel` | `-E` | Excelレポートを生成 |
| `--excel-fold-level <LEVEL>` | `-L` | Excelファイルツリーの折りたたみレベル |
| `--show-unchanged` | `-u` | 未変更ファイルも表示 |
| `--save-config <PATH>` | `-C` | 現在のオプションを設定ファイルに保存 |
| `--filter-status <STATUS>` | | ステータスフィルタ（カンマ区切り、`^`で除外） |
| `--stats-only` | | ヘッダー/Options/統計情報のみ表示 |
| `--no-tree` | | File Treeセクション非表示 |
| `--no-details` | | 詳細セクション非表示 |
| `--copy-deleted` | | 削除ファイルもコピー（.deleted） |
| `--preserve-timestamps` | | タイムスタンプを保持 |
| `--workers <NUM>` | `-j` | 並列ワーカー数 |
| `--temp-dir <PATH>` | | 一時ファイルの保存先 |
| `--color <MODE>` | | カラー出力（auto/always/never） |
| `--log-level <LEVEL>` | | ログレベル（error/warn/info/debug） |
| `--three-way` | `-3` | 三方向比較モード |
| `--base` | `-B` | 三方向比較のベースディレクトリ |
| `--merge-style` | `-M` | マージスタイル（all/ours/theirs） |
| `--conflict-only` | | コンフリクトのみ出力 |
| `--help` | `-h` | ヘルプ表示 |
| `--version` | `-V` | バージョン表示 |

> 注意: `--stats-only` は**コンソール出力のみ**に影響します。`--summary` や `--excel` には完全なサマリーが出力されます。

フィルタの有効値やグループキーワード一覧は `rs_diffcopy.md` の「出力フィルター機能」を参照してください。

## 出力例

### 二方向比較の出力

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

### 三方向比較の出力

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

## 終了コード

| コード | 意味 |
|--------|------|
| 0 | 差分あり（正常終了） |
| 1 | エラー発生 |
| 2 | 差分なし |
| 3 | コンフリクトあり（三方向比較時） |

## 設定ファイル

TOML形式の設定ファイルを使用できます。
アプリケーション共通の `settings.toml` については `rs_diffcopy.md` の「アプリケーション設定ファイル（settings.toml）」を参照してください。

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

設定ファイルの使用:

```bash
rs_diffcopy --config config.toml
```

現在のオプションを設定ファイルに保存:

```bash
rs_diffcopy -S ./old -T ./new -O ./diff -e "*.log" --save-config config.toml
```

## 高度な使い方

### 除外パターン

```bash
# ログファイルとnode_modulesを除外
rs_diffcopy -S ./old -T ./new -O ./diff -e "*.log" -e "node_modules"
```

### 出力とレポート

```bash
# サマリーをファイルに出力
rs_diffcopy -S ./old -T ./new -O ./diff --summary summary.txt

# Excelレポートの生成
rs_diffcopy -S ./old -T ./new -O ./diff --excel report.xlsx
```

### パッチ生成

```bash
# 個別のパッチファイルを生成
rs_diffcopy -S ./old -T ./new -O ./diff --patch

# 統合パッチファイルを生成
rs_diffcopy -S ./old -T ./new -O ./diff --patch-file changes.patch

# 両方を生成
rs_diffcopy -S ./old -T ./new -O ./diff --patch --patch-file changes.patch
```

### コピーオプション

```bash
# 変更ファイルの新旧両方をコピー
rs_diffcopy -S ./old -T ./new -O ./diff --both-versions

# 削除ファイルもコピー
rs_diffcopy -S ./old -T ./new -O ./diff --copy-deleted

# タイムスタンプ保持でコピー
rs_diffcopy -S ./old -T ./new -O ./diff --preserve-timestamps
```

### フィルタ・表示制御

```bash
# 追加と変更のみ表示（コピー対象もフィルタされる）
rs_diffcopy -S ./old -T ./new -O ./diff --filter-status added,modified

# 変更なし以外すべて表示（暗黙の all + 除外）
rs_diffcopy -S ./old -T ./new -O ./diff --filter-status ^unchanged

# 追加と削除以外を表示（all + 除外）
rs_diffcopy -S ./old -T ./new -O ./diff --filter-status all,^added,^deleted

# 追加のみ表示（エイリアスも可: add/a）
rs_diffcopy -S ./old -T ./new -O ./diff --filter-status add

# エラーとシンボリックリンクのみ表示
rs_diffcopy -S ./old -T ./new -O ./diff --filter-status error,symlink

# 統計情報のみ表示（コピーは通常通り）
rs_diffcopy -S ./old -T ./new -O ./diff --stats-only

# File Treeを非表示
rs_diffcopy -S ./old -T ./new -O ./diff --no-tree

# 詳細セクションを非表示
rs_diffcopy -S ./old -T ./new -O ./diff --no-details
```

> 注意: `--stats-only` は**コンソール出力のみ**に影響します。`--summary` や `--excel` には完全なサマリーが出力されます。

### 出力色とログ

```bash
# 常にカラー出力
rs_diffcopy -S ./old -T ./new -O ./diff --color always

# ログレベルをデバッグに
rs_diffcopy -S ./old -T ./new -O ./diff --log-level debug
```

### パフォーマンス調整

```bash
# ワーカー数を指定
rs_diffcopy -S ./old -T ./new -O ./diff --workers 4

# 一時ディレクトリを指定
rs_diffcopy -S ./old -T ./new -O ./diff --temp-dir /tmp/rs_diffcopy
```

### 権限チェック

```bash
# スクリプトファイルのみ権限変更チェック
rs_diffcopy -S ./old -T ./new -O ./diff --check-permissions scripts

# すべてのファイルの権限変更チェック
rs_diffcopy -S ./old -T ./new -O ./diff --check-permissions all
```

### 設定ファイルの保存

```bash
# 現在のオプションを設定ファイルに保存（差分処理も実行される）
rs_diffcopy -S ./old -T ./new -O ./diff -e "*.log" --save-config diffcopy.toml
```

### 三方向比較

```bash
# コンフリクトのみ出力
rs_diffcopy --three-way -B ./base -S ./ours -T ./theirs -O ./output --conflict-only

# コンフリクト系のみ表示（グループ指定）
rs_diffcopy --three-way -B ./base -S ./ours -T ./theirs -O ./output --filter-status conflicts

# 追加系のみ表示（added グループが展開される）
rs_diffcopy --three-way -B ./base -S ./ours -T ./theirs -O ./output --filter-status added

# 追加系を除外（暗黙の all + 除外）
rs_diffcopy --three-way -B ./base -S ./ours -T ./theirs -O ./output --filter-status ^added

# Oursを優先
rs_diffcopy --three-way -B ./base -S ./ours -T ./theirs -O ./output --merge-style ours

# Theirsを優先
rs_diffcopy --three-way -B ./base -S ./ours -T ./theirs -O ./output --merge-style theirs
```

## 動作要件

- Rust 1.70以上
- サポートOS: Linux, macOS, Windows

## ライセンス

MIT License

Copyright (c) 2024 kznagamori

## 作者

kznagamori

## リンク

- [GitHub Repository](https://github.com/kznagamori/rs_diffcopy)
- [Issues](https://github.com/kznagamori/rs_diffcopy/issues)
