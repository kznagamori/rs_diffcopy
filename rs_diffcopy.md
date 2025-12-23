# rs_diffcopy 要件定義

## 1. プロジェクト概要

- **アプリケーション名**: `rs_diffcopy`
- **バージョン**: `v1.0`
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

## 3. 機能一覧（v1.0）

| 機能 | 説明 |
|------|------|
| ディレクトリ比較 | 2つのディレクトリを比較し、差分ファイルを抽出 |
| 除外パターン | glob形式で除外ファイル/ディレクトリを指定可能 |
| 設定ファイル | TOML形式の設定ファイルで複雑な設定を再利用 |
| 新旧両方コピー | 変更ファイルの新旧両方を `.old`/`.new` 拡張子付きでコピー |
| 権限チェック | ファイル権限の変更を検出（スクリプト限定/全ファイル） |
| ドライラン | 実際にコピーせず、対象ファイルをプレビュー |
| 進捗表示 | フェーズ別プログレスバーを表示 |
| 並列処理 | ファイル比較・コピーを並列実行で高速化 |

---

## 4. CLI仕様

### 4.1 コマンドライン引数

```
rs_diffcopy [OPTIONS]

必須オプション（設定ファイル未使用時）:
  -S, --source <PATH>              比較元ディレクトリ（before）
  -T, --target <PATH>              比較先ディレクトリ（after）
  -O, --output <PATH>              差分ファイルの出力先

オプション:
  -c, --config <PATH>              設定ファイル（TOML形式）
  -e, --exclude <PATTERN>          除外パターン（複数指定可、glob形式）
  -f, --force                      出力先を全削除して再実行
  -s, --summary <PATH>             サマリーをファイルに出力（デフォルト: 標準出力）
  -v, --verbose                    詳細出力モード（処理中ファイル名を表示）
  -n, --dry-run                    実際にコピーせず、対象ファイルを表示
  -b, --both-versions              変更ファイルの新旧両方をコピー（.old/.new拡張子付与）
  -P, --check-permissions <MODE>   権限変更をチェック（none/scripts/all）
  -h, --help                       ヘルプ表示
  -V, --version                    バージョン表示
```

### 4.2 使用例

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

# 設定ファイルを使用
rs_diffcopy --config ./diffcopy.toml
```

### 4.3 設定ファイル（TOML形式）

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

#### 優先順位

コマンドライン引数と設定ファイルの両方が指定された場合、**コマンドライン引数が優先**されます。

### 4.4 除外パターン

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

## 5. 差異判定

### 5.1 ファイル内容比較

- **BLAKE3ハッシュ**によるファイル内容比較
  - 高速かつ衝突耐性が極めて高い（256bit出力）
  - SIMD最適化により高いパフォーマンス
- **最適化**: サイズが異なれば即座に「差異あり」と判定（ハッシュ計算を省略）
- サイズが同じ場合のみハッシュを計算して比較

### 5.2 権限・属性チェック

`-P/--check-permissions` オプションで、ファイルの権限・属性の変更を検出できます。

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

## 6. ファイル状態別の扱い

### 6.1 通常モード（デフォルト）

| 状態 | 扱い |
|------|------|
| 新規ファイル | 出力先にコピー |
| 変更ファイル | 出力先にコピー（target側のファイル） |
| 新規ディレクトリ（空含む） | 出力先に作成 |
| 削除ファイル/ディレクトリ | サマリーに記載のみ（コピーしない） |
| シンボリックリンク | サマリーに記載のみ（コピーしない） |
| 権限エラー | スキップしてサマリーに記載 |
| 権限変更のみ | サマリーに記載のみ（コピーしない） |

### 6.2 --both-versions モード

| 状態 | 扱い |
|------|------|
| 新規ファイル | 出力先にコピー（拡張子なし） |
| 変更ファイル | 新旧両方をコピー（`filename.ext.old` / `filename.ext.new`） |
| その他 | 通常モードと同じ |

---

## 7. 出力構造

### 7.1 通常モード

```
output_dir/
├── src/
│   ├── main.rs          # 変更ファイル
│   └── new_feature.rs   # 新規ファイル
├── docs/                # 新規ディレクトリ（空でも作成）
└── config.toml          # 変更ファイル
```

### 7.2 --both-versions モード

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

---

## 8. 進捗表示

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

### 8.1 処理フェーズ

| フェーズ | 処理内容 | 並列化 |
|---------|---------|:------:|
| Phase 1 | Scanning (ディレクトリ走査) | - |
| Phase 2 | Comparing (ハッシュ比較) | 並列 |
| Phase 3 | Copying (ファイルコピー) | 並列 |
| Phase 4 | Writing summary | - |

### 8.2 表示仕様

- 各フェーズ開始時に `[n/4]` 形式でフェーズ番号を表示
- 比較・コピー処理中はプログレスバーで進捗を表示
- パイプやリダイレクト時はプログレスバーを非表示
- `--verbose` モードでは追加の詳細情報を表示

---

## 9. サマリー出力形式

### 9.1 差分ありの場合

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

Added:       5 files, 1 dir
Modified:    8 files
Deleted:     2 files, 1 dir
Symlinks:    3 files
Permissions: 2 files
Errors:      1 file
--------------------------
Total:      22 items

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
├── cache -> /tmp/cache [symlink: added]
├── broken_link -> /nonexistent [symlink: added, broken]
├── old_link -> /old/path [symlink: deleted]
├── changed_link -> /new/path [symlink: changed]
├── became_broken -> /path [symlink: broken]
├── secret.key [permission denied]
└── legacy/ [deleted]

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
```

### 9.2 差分なしの場合

```
rs_diffcopy Summary
================
Source: /path/to/source
Target: /path/to/target
Output: /path/to/output
Date: 2025-12-18 10:30:00

No differences found.
```

### 9.3 サマリーセクション一覧

| セクション | 表示条件 | 内容 |
|------------|----------|------|
| ヘッダー | 常に表示 | Source, Target, Output, Date |
| Options | オプション指定時 | 使用したオプションの一覧 |
| 統計情報 | 常に表示 | Added, Modified, Deleted等の件数 |
| File Tree | 差分あり時 | ツリー形式の差分一覧 |
| Added Files | 追加あり時 | 追加されたファイル/ディレクトリ一覧 |
| Modified Files | 変更あり時 | 変更されたファイル一覧 |
| Deleted Files | 削除あり時 | 削除されたファイル/ディレクトリ一覧 |
| Symlink Details | シンボリックリンクあり時 | シンボリックリンクの詳細情報 |
| Permission Changes | 権限変更あり時 | 権限変更の詳細 |
| Errors | エラーあり時 | エラー詳細 |

### 9.4 ステータスタグ一覧

| タグ | 意味 |
|------|------|
| `[added]` | 新規追加 |
| `[modified]` | 内容変更あり |
| `[deleted]` | 削除（コピーされない） |
| `[symlink: added]` | 新規追加されたシンボリックリンク |
| `[symlink: added, broken]` | 新規追加された壊れたシンボリックリンク |
| `[symlink: deleted]` | 削除されたシンボリックリンク |
| `[symlink: changed]` | リンク先が変更されたシンボリックリンク |
| `[symlink: changed, broken]` | リンク先が変更され、かつ壊れたシンボリックリンク |
| `[symlink: broken]` | 同じリンク先で壊れた状態になったシンボリックリンク |
| `[permission denied]` | 権限エラー（スキップ） |

※ シンボリックリンクはコピーされず、Symlink Detailsセクションに詳細が表示される
※ 権限変更は File Tree には表示されず、Permission Changes セクションにのみ表示

---

## 10. 終了コード

| コード | 意味 | 説明 |
|:------:|------|------|
| 0 | 正常終了（差分あり） | 差分が検出され、正常に処理完了 |
| 1 | エラー終了 | ディレクトリが存在しない、権限不足など |
| 2 | 正常終了（差分なし） | 比較対象に差分がなかった |

---

## 11. エラーハンドリング

| ケース | 挙動 |
|--------|------|
| source/targetディレクトリが存在しない | エラー終了（コード1） |
| 出力先が既に存在 | エラー終了（`--force`で全削除して実行可能） |
| ファイル読み取り権限エラー | スキップしてサマリーに記載、処理は継続 |
| シンボリックリンク | スキップしてサマリーに記載、処理は継続 |
| 設定ファイルの必須項目不足 | エラー終了（コード1） |
| 無効なglob パターン | エラー終了（コード1） |

---

## 12. 文字エンコーディング

| 条件 | エンコーディング |
|------|-----------------|
| Windows + コンソール出力 | CP932 (Shift-JIS) |
| Windows + パイプ/リダイレクト | UTF-8 |
| Linux/macOS | UTF-8 |
| サマリーファイル (`-s`) | UTF-8 |

### 日本語対応

- 日本語ファイルパスを正しく処理
- Windowsコンソールでは自動的にCP932に変換して文字化けを防止
- パイプやファイルリダイレクト時はUTF-8で出力

---

## 13. 将来の機能検討

### 13.1 v1.1 で検討する機能（優先度: 高）

| 機能 | オプション案 | 説明 | 対象ユーザーへの価値 |
|------|-------------|------|---------------------|
| 削除ファイルのコピー | `--copy-deleted` | source側にしかないファイルも出力先に抽出 | 変更前後の完全な差分を確認したい場合に有用 |
| タイムスタンプ保持 | `--preserve-timestamps` | コピー時にファイルの更新日時を保持 | ファイル履歴の追跡に有用 |
| カラー出力 | `--color` / `--no-color` | ターミナルで色付き出力（added=緑, deleted=赤, modified=黄） | 視認性向上、初心者にも分かりやすい |

### 13.2 v1.2 以降で検討する機能（優先度: 中）

| 機能 | オプション案 | 説明 |
|------|-------------|------|
| インクルードパターン | `-i, --include <PATTERN>` | 特定のファイルのみを対象にする（excludeの逆） |
| 日付フィルタ | `--newer-than <DATE>` | 指定日以降に変更されたファイルのみ対象 |
| サイズフィルタ | `--min-size`, `--max-size` | 最小/最大サイズでフィルタ |
| JSON出力 | `--json` | 機械可読なJSON形式でサマリー出力 |
| 静音モード | `-q, --quiet` | 最小限の出力のみ（スクリプト向け） |

### 13.3 将来検討する機能（優先度: 低）

| 機能 | 説明 |
|------|------|
| ハッシュキャッシュ | 前回の比較結果をキャッシュし、増分比較で高速化 |
| HTMLレポート | ブラウザで閲覧可能なレポート生成 |
| 差分パッチ生成 | unified diff形式の出力 |
| ウォッチモード | ディレクトリの変更を監視して自動実行 |

### 13.4 v1.0 機能充足度

v1.0は想定ユーザー（初心者・非技術者）に対して以下の点で十分な機能を提供：

- **シンプルなCLI**: 最小限のオプションで基本機能を利用可能
- **分かりやすい出力**: ツリー形式のサマリーで変更点を視覚的に把握
- **安全な操作**: ドライランモードで事前確認、`--force`なしでは上書きしない
- **再利用性**: TOML設定ファイルで複雑な設定を保存・再利用
- **クロスプラットフォーム**: Windows/Linux/macOSで同一の動作
