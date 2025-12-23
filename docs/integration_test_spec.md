# DiffCopy 結合テスト仕様書

## 1. 概要

本仕様書は、DiffCopy CLIツールの結合テストを定義する。
結合テストでは、実際にCLIコマンドを実行し、出力結果、終了コード、ファイル操作の正確性を検証する。

---

## 2. テスト環境

- テスト用の一時ディレクトリを使用
- 各テストは独立して実行可能
- テスト終了後、一時ディレクトリは自動削除

---

## 3. テストケース一覧

### 3.1 基本機能テスト

| ID | テスト名 | 説明 | 期待結果 |
|----|---------|------|----------|
| IT-001 | 基本的な差分検出 | source/targetに差分がある場合の動作確認 | 差分ファイルがoutputにコピーされる |
| IT-002 | 差分なし | source/targetが同一の場合 | 終了コード2、"No differences found."出力 |
| IT-003 | 新規ファイル検出 | targetにのみ存在するファイル | [added]として検出、コピーされる |
| IT-004 | 変更ファイル検出 | 両方に存在し内容が異なるファイル | [modified]として検出、コピーされる |
| IT-005 | 削除ファイル検出 | sourceにのみ存在するファイル | [deleted]として検出、コピーされない |
| IT-006 | 空ディレクトリ検出 | targetにのみ存在する空ディレクトリ | [added]として検出、作成される |

### 3.2 オプションテスト

| ID | テスト名 | 説明 | 期待結果 |
|----|---------|------|----------|
| IT-101 | --helpオプション | ヘルプ表示 | Usage情報が出力される |
| IT-102 | --versionオプション | バージョン表示 | "diffcopy 1.0.0"が出力される |
| IT-103 | --verboseオプション | 詳細出力モード | 処理中ファイル名が出力される |
| IT-104 | --dry-runオプション | ドライラン | ファイルがコピーされない |
| IT-105 | --forceオプション | 出力先強制削除 | 既存出力先が削除され再実行される |
| IT-106 | --summaryオプション | サマリーファイル出力 | 指定パスにサマリーが保存される |
| IT-107 | --excludeオプション | 除外パターン | 指定パターンのファイルが除外される |
| IT-108 | 複数--excludeオプション | 複数除外パターン | 全パターンが適用される |

### 3.3 エラーハンドリングテスト

| ID | テスト名 | 説明 | 期待結果 |
|----|---------|------|----------|
| IT-201 | 存在しないsourceディレクトリ | sourceが存在しない | 終了コード1、エラーメッセージ |
| IT-202 | 存在しないtargetディレクトリ | targetが存在しない | 終了コード1、エラーメッセージ |
| IT-203 | 出力先が既に存在 | --forceなしで出力先が存在 | 終了コード1、エラーメッセージ |
| IT-204 | 無効な除外パターン | 不正なglobパターン | 終了コード1、エラーメッセージ |

### 3.4 終了コードテスト

| ID | テスト名 | 説明 | 期待結果 |
|----|---------|------|----------|
| IT-301 | 正常終了（差分あり） | 差分があり正常終了 | 終了コード0 |
| IT-302 | 正常終了（差分なし） | 差分がなく正常終了 | 終了コード2 |
| IT-303 | エラー終了 | エラー発生時 | 終了コード1 |

### 3.5 サマリー出力テスト

| ID | テスト名 | 説明 | 期待結果 |
|----|---------|------|----------|
| IT-401 | サマリーヘッダー | ヘッダー情報の確認 | Source/Target/Date情報が含まれる |
| IT-402 | ファイルツリー出力 | ツリー形式の出力確認 | 正しいツリー構造で出力される |
| IT-403 | ステータスタグ | 各ステータスのタグ確認 | [added]/[modified]/[deleted]が正しく付与 |
| IT-404 | 統計情報 | 件数の統計情報 | Added/Modified/Deleted/Totalが正確 |

### 3.6 ファイル操作テスト

| ID | テスト名 | 説明 | 期待結果 |
|----|---------|------|----------|
| IT-501 | ディレクトリ階層維持 | 深いネストのファイルコピー | 階層構造が維持される |
| IT-502 | バイナリファイルコピー | バイナリファイルの正確なコピー | 内容が完全一致 |
| IT-503 | 大きなファイル | 大きなファイルの処理 | 正常に処理される |
| IT-504 | 多数のファイル | 多数のファイルの処理 | 全ファイルが正しく処理される |

### 3.7 シンボリックリンクテスト（Linuxのみ）

| ID | テスト名 | 説明 | 期待結果 |
|----|---------|------|----------|
| IT-601 | シンボリックリンク追加検出 | targetにのみ存在するsymlink | [symlink: added]として検出、コピーされない |
| IT-602 | シンボリックリンク詳細 | symlink詳細情報 | リンク先情報がサマリーに記載 |
| IT-603 | シンボリックリンク削除検出 | sourceにのみ存在するsymlink | [symlink: deleted]として検出 |
| IT-604 | シンボリックリンク変更検出 | リンク先が変更されたsymlink | [symlink: changed]として検出 |
| IT-605 | 壊れたシンボリックリンク検出 | 存在しないパスを指すsymlink | [symlink: added, broken]として検出 |
| IT-606 | シンボリックリンク壊れ検出 | 同じターゲットでリンク先が消失 | [symlink: broken]として検出 |
| IT-607 | 通常ファイル→symlink変換検出 | 通常ファイルがsymlinkに変更 | [symlink: added]として検出 |
| IT-608 | symlink→通常ファイル変換検出 | symlinkが通常ファイルに変更 | [symlink: deleted]として検出 |
| IT-609 | symlink変更かつ壊れ検出 | リンク先変更＋リンク先が存在しない | [symlink: changed, broken]として検出 |

### 3.8 並列処理テスト

| ID | テスト名 | 説明 | 期待結果 |
|----|---------|------|----------|
| IT-701 | 並列比較処理 | 多数ファイルの並列比較 | 全ファイルが正確に比較される |
| IT-702 | 並列コピー処理 | 多数ファイルの並列コピー | 全ファイルが正確にコピーされる |
| IT-703 | 進捗表示 | フェーズ別進捗表示 | [1/4]〜[4/4]の進捗が表示される |

---

## 4. テストデータ構造

### 4.1 基本テストデータ

```
source/
├── src/
│   ├── main.rs         # 変更対象ファイル
│   └── lib.rs          # 未変更ファイル
├── docs/
│   └── readme.md       # 削除対象ファイル
└── config.toml         # 削除対象ファイル

target/
├── src/
│   ├── main.rs         # 内容変更
│   ├── lib.rs          # 同一内容
│   └── new_module.rs   # 新規ファイル
├── tests/              # 新規ディレクトリ
│   └── test_main.rs    # 新規ファイル
└── new_config.toml     # 新規ファイル
```

### 4.2 期待される出力

```
output/
├── src/
│   ├── main.rs         # [modified]
│   └── new_module.rs   # [added]
├── tests/              # [added]
│   └── test_main.rs    # [added]
└── new_config.toml     # [added]
```

### 4.3 シンボリックリンクテストデータ（Linux）

```
# IT-601: 追加検出
source/
└── (empty)

target/
└── link.txt -> real.txt    # 新規symlink

# IT-603: 削除検出
source/
└── link.txt -> real.txt    # 削除対象symlink

target/
└── (empty)

# IT-604: 変更検出
source/
└── link.txt -> old_target.txt

target/
└── link.txt -> new_target.txt  # リンク先変更

# IT-605: 壊れたリンク検出（新規追加）
target/
└── broken.txt -> nonexistent.txt  # 存在しないパス

# IT-606: シンボリックリンク壊れ検出（同じターゲット）
source/
├── target.txt                     # 実ファイル
└── link.txt -> target.txt         # 動作するsymlink

target/
└── link.txt -> target.txt         # target.txtがないので壊れている

# IT-607: 通常ファイル→symlink変換
source/
└── file.txt                       # 通常ファイル

target/
├── target.txt
└── file.txt -> target.txt         # symlinkに変換

# IT-608: symlink→通常ファイル変換
source/
├── target.txt
└── file.txt -> target.txt         # symlink

target/
└── file.txt                       # 通常ファイルに変換

# IT-609: symlink変更かつ壊れ
source/
├── old_target.txt
└── link.txt -> old_target.txt     # 動作するsymlink

target/
└── link.txt -> new_target.txt     # リンク先変更＋存在しない
```

### 4.4 シンボリックリンク詳細出力形式

```
================
Symlink Details
================
Added:
  new_link.txt -> target.txt
  another.txt -> dest.txt (BROKEN)

Deleted:
  old_link.txt -> removed_target.txt

Changed:
  changed_link.txt
    Before: old_target.txt (file, OK)
    After:  new_target.txt (file, BROKEN)
  broken_link.txt
    Before: target.txt (file, OK)
    After:  target.txt (file, BROKEN)
```

### 4.5 シンボリックリンクステータスタグ

| タグ | 意味 |
|------|------|
| `[symlink: added]` | 新規追加されたシンボリックリンク |
| `[symlink: added, broken]` | 新規追加された壊れたシンボリックリンク |
| `[symlink: deleted]` | 削除されたシンボリックリンク |
| `[symlink: changed]` | リンク先が変更されたシンボリックリンク |
| `[symlink: changed, broken]` | リンク先変更＋壊れたシンボリックリンク |
| `[symlink: broken]` | 同じリンク先で壊れた状態に変化 |

---

## 5. 実行方法

```bash
# 全結合テスト実行
cargo test --test integration_tests

# 特定のテスト実行
cargo test --test integration_tests test_basic_diff_detection

# 詳細出力付き
cargo test --test integration_tests -- --nocapture
```

---

## 6. 合否判定基準

- 全テストケースが成功すること
- 終了コードが期待値と一致すること
- 出力ファイルの内容が正確であること
- サマリー出力が仕様通りであること
