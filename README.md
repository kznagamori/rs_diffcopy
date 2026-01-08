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

## 主要なオプション

| オプション | 短縮形 | 説明 |
|-----------|--------|------|
| `--source` | `-S` | ソースディレクトリ |
| `--target` | `-T` | ターゲットディレクトリ |
| `--output` | `-O` | 出力ディレクトリ |
| `--config` | `-c` | 設定ファイルのパス |
| `--exclude` | `-e` | 除外パターン（glob形式、複数指定可） |
| `--force` | `-f` | 出力ディレクトリを強制上書き |
| `--dry-run` | `-n` | ドライラン（実際にコピーしない） |
| `--both-versions` | `-b` | 新旧両バージョンをコピー（.old/.new） |
| `--patch` | `-p` | 個別パッチファイルを生成 |
| `--patch-file` | `-F` | 統合パッチファイルを生成 |
| `--excel` | `-E` | Excelレポートを生成 |
| `--summary` | `-s` | サマリーをファイルに保存 |
| `--verbose` | `-v` | 詳細出力モード |
| `--show-unchanged` | `-u` | 未変更ファイルも表示 |
| `--three-way` | `-3` | 三方向比較モード |
| `--base` | `-B` | 三方向比較のベースディレクトリ |
| `--merge-style` | `-M` | マージスタイル（all/ours/theirs） |
| `--conflict-only` | | コンフリクトのみ出力 |

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

### 特定のファイルタイプのみ比較

```bash
# ログファイルとnode_modulesを除外
rs_diffcopy -S ./old -T ./new -O ./diff -e "*.log" -e "node_modules"
```

### パッチファイルの生成

```bash
# 個別のパッチファイルを生成
rs_diffcopy -S ./old -T ./new -O ./diff --patch

# 統合パッチファイルを生成
rs_diffcopy -S ./old -T ./new -O ./diff --patch-file changes.patch

# 両方を生成
rs_diffcopy -S ./old -T ./new -O ./diff --patch --patch-file changes.patch
```

### Excelレポートの生成

```bash
rs_diffcopy -S ./old -T ./new -O ./diff --excel report.xlsx
```

### 三方向比較でコンフリクトのみ出力

```bash
rs_diffcopy --three-way -B ./base -S ./ours -T ./theirs -O ./output --conflict-only
```

### マージスタイルの指定

```bash
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
