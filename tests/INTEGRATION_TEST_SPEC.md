# rs_diffcopy 結合テスト仕様書

## 概要

このドキュメントは、rs_diffcopyの結合テスト仕様を定義します。
結合テストは、コマンドライン引数を使用した実際の動作をテストします。

## テスト結果の記録

テスト実行後は、以下のファイルに結果を記録してください：

```
tests/TEST_RESULTS.md
```

### 結果ファイルの更新方法

```bash
# テスト実行
cargo test --test integration_tests -- --test-threads=1 2>&1 | tee test_output.txt

# 結果をTEST_RESULTS.mdに記録
# - 実施日
# - 実行環境
# - テスト結果（PASS/FAIL）
# - 失敗時のエラー詳細
```

## テスト環境

- テスト用の一時ディレクトリを使用
- 各テストは独立して実行可能
- 日本語パスのテストを含む

## テストカテゴリと詳細

---

### 1. 基本動作テスト

| テストID | テスト名 | テスト内容 | 期待結果 | 分類 |
|---------|---------|-----------|---------|------|
| BASIC-001 | test_help_option | `--help`オプション実行 | ヘルプが表示され、終了コード0 | 正常系 |
| BASIC-002 | test_help_short_option | `-h`オプション実行 | ヘルプが表示され、終了コード0 | 正常系 |
| BASIC-003 | test_version_option | `--version`オプション実行 | バージョン表示、終了コード0 | 正常系 |
| BASIC-004 | test_version_short_option | `-V`オプション実行 | バージョン表示、終了コード0 | 正常系 |

---

### 2. 二方向比較テスト

| テストID | テスト名 | テスト内容 | 期待結果 | 分類 |
|---------|---------|-----------|---------|------|
| TWO-001 | test_added_file_detection | ターゲットのみにファイルが存在 | 「Added」として検出、出力にコピー | 正常系 |
| TWO-002 | test_modified_file_detection | ソースとターゲットで内容が異なる | 「Modified」として検出、新内容をコピー | 正常系 |
| TWO-003 | test_deleted_file_detection | ソースのみにファイルが存在 | 「Deleted」として検出、コピーされない | 正常系 |
| TWO-004 | test_unchanged_file_detection | ソースとターゲットで内容が同一 | 「Unchanged」として検出、コピーされない | 正常系 |
| TWO-005 | test_directory_structure_preserved | ネストしたディレクトリ構造 | ディレクトリ構造が出力に保持 | 正常系 |
| TWO-006 | test_no_differences_exit_code | 差分なし | 終了コード2 | 正常系 |
| TWO-007 | test_differences_exit_code | 差分あり | 終了コード0 | 正常系 |
| TWO-008 | test_empty_directories | 空のソースとターゲット | 正常終了、終了コード2 | 準正常系 |
| TWO-009 | test_large_file_comparison | 大きなファイル（1MB以上） | 正常に比較・コピー | 準正常系 |
| TWO-010 | test_binary_file_detection | バイナリファイルの比較 | バイナリとして正しく検出・コピー | 準正常系 |
| TWO-011 | test_symlink_handling | シンボリックリンクの処理 | 正しく処理（Unix系のみ） | 準正常系 |
| TWO-012 | test_special_characters_in_filename | 特殊文字を含むファイル名 | 正しく処理 | 準正常系 |
| TWO-013 | test_deeply_nested_directories | 深いネスト（10階層以上） | 正しく処理 | 準正常系 |
| TWO-014 | test_many_files | 多数ファイル（100以上） | 正しく処理 | 準正常系 |
| TWO-015 | test_empty_file | 空ファイルの比較 | 正しく検出 | 準正常系 |

---

### 3. 三方向比較テスト

| テストID | テスト名 | テスト内容 | 期待結果 | 分類 |
|---------|---------|-----------|---------|------|
| THREE-001 | test_three_way_basic | 基本的な三方向比較 | 正常に動作 | 正常系 |
| THREE-002 | test_three_way_conflict | 両方で異なる変更 | コンフリクト検出、終了コード3 | 正常系 |
| THREE-003 | test_three_way_both_same_change | 両方で同一の変更 | 「both-same」として検出 | 正常系 |
| THREE-004 | test_three_way_requires_base | --baseなしで--three-way | エラー終了 | 準正常系 |
| THREE-005 | test_three_way_ours_only_change | Oursのみ変更 | ours-onlyとして検出 | 正常系 |
| THREE-006 | test_three_way_theirs_only_change | Theirsのみ変更 | theirs-onlyとして検出 | 正常系 |
| THREE-007 | test_three_way_file_added_both | 両方で新規追加（異なる内容） | コンフリクト検出 | 準正常系 |
| THREE-008 | test_three_way_file_deleted_both | 両方で削除 | 正常検出 | 準正常系 |

---

### 4. オプションテスト

| テストID | テスト名 | テスト内容 | 期待結果 | 分類 |
|---------|---------|-----------|---------|------|
| OPT-001 | test_dry_run | --dry-runオプション | ファイルがコピーされない | 正常系 |
| OPT-002 | test_both_versions | --both-versionsオプション | .oldと.newが作成 | 正常系 |
| OPT-003 | test_copy_deleted | --copy-deletedオプション | .deletedファイルが作成 | 正常系 |
| OPT-004 | test_preserve_timestamps | --preserve-timestampsオプション | タイムスタンプが保持 | 正常系 |
| OPT-005 | test_exclude_pattern | --excludeオプション（単一） | パターンに一致するファイルが除外 | 正常系 |
| OPT-006 | test_multiple_exclude_patterns | --excludeオプション（複数） | 複数パターンが除外 | 正常系 |
| OPT-007 | test_patch_generation | --patchオプション | .patchファイルが生成 | 正常系 |
| OPT-008 | test_combined_patch_file | --patch-fileオプション | 統合パッチファイルが生成 | 正常系 |
| OPT-009 | test_excel_report | --excelオプション | .xlsxファイルが生成 | 正常系 |
| OPT-010 | test_summary_file | --summaryオプション | サマリーファイルが生成 | 正常系 |
| OPT-011 | test_verbose_mode | --verboseオプション | 詳細出力 | 正常系 |
| OPT-012 | test_stats_only | --stats-onlyオプション | ツリー/詳細非表示、統計のみ表示 | 正常系 |
| OPT-013 | test_filter_status_added | --filter-status added | Addedファイルのみコピー | 正常系 |
| OPT-014 | test_filter_status_modified | --filter-status modified | Modifiedファイルのみコピー | 正常系 |
| OPT-015 | test_force_option | --forceオプション（dry-run併用） | 確認なしで続行 | 準正常系 |
| OPT-016 | test_no_tree_option | --no-treeオプション | ツリー表示なし | 正常系 |
| OPT-017 | test_no_details_option | --no-detailsオプション | 詳細表示なし | 正常系 |
| OPT-018 | test_workers_option | --workersオプション | 指定ワーカー数で並列処理 | 準正常系 |

---

### 5. 日本語パステスト

| テストID | テスト名 | テスト内容 | 期待結果 | 分類 |
|---------|---------|-----------|---------|------|
| JP-001 | test_japanese_directory_names | 日本語ディレクトリ名 | 正しく処理 | 正常系 |
| JP-002 | test_japanese_file_names | 日本語ファイル名 | 正しく処理・コピー | 正常系 |
| JP-003 | test_japanese_nested_path | 深いネストの日本語パス | 正しく処理 | 正常系 |
| JP-004 | test_mixed_japanese_english_path | 日本語と英語混合パス | 正しく処理 | 正常系 |
| JP-005 | test_japanese_in_summary_output | サマリーに日本語表示 | 正しく表示 | 正常系 |
| JP-006 | test_japanese_both_versions | 日本語ファイルの両バージョン | .old/.newが正しく作成 | 正常系 |
| JP-007 | test_japanese_copy_deleted | 日本語ファイルの削除コピー | .deletedが正しく作成 | 正常系 |
| JP-008 | test_japanese_patch_generation | 日本語ファイルのパッチ生成 | パッチに日本語が正しく含まれる | 正常系 |
| JP-009 | test_japanese_excel_report | 日本語パスでExcel生成 | .xlsxが正しく生成 | 正常系 |
| JP-010 | test_japanese_summary_file | 日本語パスでサマリー生成 | サマリーファイルが生成 | 正常系 |
| JP-011 | test_japanese_three_way | 日本語パスで三方向比較 | 正しく比較 | 正常系 |
| JP-012 | test_japanese_content_in_file | ファイル内容に日本語 | 正しく比較・コピー | 正常系 |
| JP-013 | test_hiragana_katakana_kanji_mixed | ひらがな・カタカナ・漢字混合 | 正しく処理 | 準正常系 |
| JP-014 | test_long_japanese_filename | 長い日本語ファイル名 | 正しく処理 | 準正常系 |

---

### 6. エラーハンドリングテスト

| テストID | テスト名 | テスト内容 | 期待結果 | 分類 |
|---------|---------|-----------|---------|------|
| ERR-001 | test_nonexistent_source | 存在しないソースディレクトリ | 終了コード1、エラーメッセージ | 異常系 |
| ERR-002 | test_nonexistent_target | 存在しないターゲットディレクトリ | 終了コード1、エラーメッセージ | 異常系 |
| ERR-003 | test_output_exists_without_force | 出力ディレクトリが既存（--forceなし） | 終了コード1、エラーメッセージ | 準正常系 |
| ERR-004 | test_invalid_glob_pattern | 不正なglobパターン | エラー終了 | 異常系 |
| ERR-005 | test_permission_denied_source | ソースに読み取り権限なし | エラー処理 | 準正常系 |
| ERR-006 | test_permission_denied_output | 出力先に書き込み権限なし | エラー処理 | 準正常系 |
| ERR-007 | test_missing_required_args | 必須引数なし | エラー終了、使用方法表示 | 異常系 |
| ERR-008 | test_conflicting_options | 矛盾するオプションの組み合わせ | 適切なエラー処理 | 準正常系 |

---

### 7. 終了コードテスト

| テストID | テスト名 | テスト内容 | 期待結果 | 分類 |
|---------|---------|-----------|---------|------|
| EXIT-001 | test_exit_code_0_with_differences | 差分あり | 終了コード0 | 正常系 |
| EXIT-002 | test_exit_code_1_on_error | エラー発生 | 終了コード1 | 異常系 |
| EXIT-003 | test_exit_code_2_no_differences | 差分なし | 終了コード2 | 正常系 |
| EXIT-004 | test_exit_code_3_conflicts | コンフリクトあり | 終了コード3 | 正常系 |

---

### 8. 保護ディレクトリテスト（ユニットテストのみ）

**注意**: このテストは実際のシステムディレクトリを誤って削除するリスクがあるため、
結合テストではなくユニットテスト（`src/safety.rs`）で検証します。

| テストID | テスト名 | テスト内容 | 期待結果 | 備考 |
|---------|---------|-----------|---------|------|
| PROT-001 | test_is_protected_path_unix_root | /が保護されるか | 保護される | ユニットテスト |
| PROT-002 | test_is_protected_path_unix_system_dirs | /etc等が保護されるか | 保護される | ユニットテスト |
| PROT-003 | test_is_protected_path_unix_safe_paths | 一時ディレクトリは保護されないか | 保護されない | ユニットテスト |
| PROT-004 | test_check_output_directory_not_exists | 存在しない出力先 | OKを返す | ユニットテスト |
| PROT-005 | test_check_output_directory_exists_no_force | 既存出力先（force無） | エラーを返す | ユニットテスト |
| PROT-006 | test_check_output_directory_exists_dry_run | 既存出力先（dry-run） | 削除しない | ユニットテスト |

---

### 9. エッジケーステスト（実運用不具合対応）

| テストID | テスト名 | テスト内容 | 期待結果 | 分類 |
|---------|---------|-----------|---------|------|
| EDGE-001 | test_empty_directory_detection | 空ディレクトリの検出 | ディレクトリがコピーされる | 準正常系 |
| EDGE-002 | test_empty_directories_both_sides | 両側に空ディレクトリ | 差分なし、終了コード2 | 準正常系 |
| EDGE-003 | test_same_size_different_content | 同サイズで異なる内容 | Modifiedとして検出 | 準正常系 |
| EDGE-004 | test_unicode_russian_filenames | ロシア語ファイル名 | 正しく処理 | 準正常系 |
| EDGE-005 | test_exclude_pycache_pattern | __pycache__除外パターン | 正しく除外 | 準正常系 |
| EDGE-006 | test_both_versions_added_files_unchanged | 追加ファイルでboth-versions | 拡張子なしでコピー | 準正常系 |

---

### 10. 設定ファイルテスト

| テストID | テスト名 | テスト内容 | 期待結果 | 分類 |
|---------|---------|-----------|---------|------|
| CFG-001 | test_config_file_basic | 基本的な設定ファイル | 設定が読み込まれる | 正常系 |
| CFG-002 | test_config_file_with_exclude | excludeパターン指定 | 除外パターンが適用 | 正常系 |
| CFG-003 | test_config_file_with_both_versions | both_versions指定 | 両バージョンが作成 | 正常系 |
| CFG-004 | test_cli_overrides_config | CLI引数でオーバーライド | CLI引数が優先 | 正常系 |

---

### 11. show_unchangedテスト

| テストID | テスト名 | テスト内容 | 期待結果 | 分類 |
|---------|---------|-----------|---------|------|
| SHOW-001 | test_show_unchanged_option | --show-unchangedオプション | 未変更ファイルが表示 | 正常系 |
| SHOW-002 | test_show_unchanged_short_option | -uオプション | 未変更ファイルが表示 | 正常系 |
| SHOW-003 | test_unchanged_count_in_statistics | 統計に未変更数表示 | 常に未変更数が表示 | 正常系 |

---

### 12. シンボリックリンクテスト（Unix only）

| テストID | テスト名 | テスト内容 | 期待結果 | 分類 |
|---------|---------|-----------|---------|------|
| SYM-001 | test_symlink_added_detection | シンボリックリンク追加 | リンクが検出される | 準正常系 |
| SYM-002 | test_symlink_deleted_detection | シンボリックリンク削除 | 削除が検出される | 準正常系 |
| SYM-003 | test_broken_symlink_detection | 壊れたシンボリックリンク | 壊れたリンクとして検出 | 準正常系 |

---

### 13. パーミッションテスト（Unix only）

| テストID | テスト名 | テスト内容 | 期待結果 | 分類 |
|---------|---------|-----------|---------|------|
| PERM-001 | test_permission_check_scripts_mode | -P scriptsオプション | パーミッション変更が検出 | 準正常系 |
| PERM-002 | test_permission_check_default_disabled | パーミッションチェックなし | デフォルトで無効 | 準正常系 |

---

### 14. 拡張三方向比較テスト

| テストID | テスト名 | テスト内容 | 期待結果 | 分類 |
|---------|---------|-----------|---------|------|
| EXT3-001 | test_three_way_added_ours | Oursのみ追加 | added-oursとして検出 | 正常系 |
| EXT3-002 | test_three_way_added_theirs | Theirsのみ追加 | added-theirsとして検出 | 正常系 |
| EXT3-003 | test_three_way_deleted_ours | Oursで削除 | deleted-oursとして検出 | 正常系 |
| EXT3-004 | test_three_way_deleted_theirs | Theirsで削除 | deleted-theirsとして検出 | 正常系 |
| EXT3-005 | test_three_way_deleted_both | 両方で削除 | deleted-bothとして検出 | 正常系 |
| EXT3-006 | test_three_way_modify_delete_conflict | 変更と削除の競合 | CONFLICTとして検出 | 準正常系 |
| EXT3-007 | test_three_way_merge_style_ours | --merge-style ours | Ours版のみコピー | 準正常系 |
| EXT3-008 | test_three_way_merge_style_theirs | --merge-style theirs | Theirs版のみコピー | 準正常系 |
| EXT3-009 | test_three_way_conflict_only | --conflict-only | 競合ファイルのみコピー | 準正常系 |

---

### 15. Excelファイル内容検証テスト

**使用クレート**: calamine（Excelファイル読み取り）

| テストID | テスト名 | テスト内容 | 期待結果 | 分類 |
|---------|---------|-----------|---------|------|
| EXCEL-001 | test_excel_has_correct_sheets | シート名の検証 | Summary, File Tree, Detailsシートが存在 | 正常系 |
| EXCEL-002 | test_excel_summary_statistics | Summaryシートの統計 | Added/Modified数が正しい | 正常系 |
| EXCEL-003 | test_excel_file_tree_entries | File Treeシートのエントリ | ファイルとステータスが正しい | 正常系 |
| EXCEL-004 | test_excel_details_sections | Detailsシートのセクション | Added/Modified/Deleted Files各セクションが存在 | 正常系 |
| EXCEL-005 | test_excel_japanese_filenames | 日本語ファイル名 | Excelに日本語が正しく記録 | 正常系 |
| EXCEL-006 | test_excel_subdirectory_structure | サブディレクトリ構造 | ディレクトリとファイル名が正しく分離 | 正常系 |

---

### 16. サマリーファイル内容検証テスト

| テストID | テスト名 | テスト内容 | 期待結果 | 分類 |
|---------|---------|-----------|---------|------|
| SUM-001 | test_summary_contains_header | ヘッダー情報 | タイトル、Source、Target、Output、Date含む | 正常系 |
| SUM-002 | test_summary_statistics_accuracy | 統計の正確性 | Added/Modified/Deleted数が正しい | 正常系 |
| SUM-003 | test_summary_contains_file_tree | ファイルツリー | File Treeセクション、ファイル名、ステータスタグ含む | 正常系 |
| SUM-004 | test_summary_contains_details | 詳細セクション | Modified Filesセクションとファイル名含む | 正常系 |
| SUM-005 | test_summary_japanese_paths | 日本語パス | 日本語のパス・ファイル名が正しく表示 | 正常系 |
| SUM-006 | test_summary_options_section | オプションセクション | Options、Dry run、Exclude patterns含む | 正常系 |
| SUM-007 | test_summary_no_differences | 差分なし時 | "No differences found"メッセージ表示 | 準正常系 |
| SUM-008 | test_summary_subdirectory_tree | サブディレクトリツリー | 深い階層のディレクトリ・ファイルが表示 | 正常系 |

---

### 17. パッチファイル内容検証テスト

| テストID | テスト名 | テスト内容 | 期待結果 | 分類 |
|---------|---------|-----------|---------|------|
| PATCH-001 | test_patch_file_unified_format | unified diff形式 | --- a/, +++ b/, @@, -/+行含む | 正常系 |
| PATCH-002 | test_combined_patch_file | 統合パッチファイル | 複数ファイルのパッチが1ファイルに | 正常系 |
| PATCH-003 | test_patch_addition_only | 追加のみ | +行のみ、-行なし | 正常系 |
| PATCH-004 | test_patch_deletion_only | 削除のみ | -行のみ、+行なし | 正常系 |
| PATCH-005 | test_patch_multiple_hunks | 複数ハンク | 離れた変更で複数の@@ヘッダー | 正常系 |
| PATCH-006 | test_patch_subdirectory | サブディレクトリ | サブディレクトリにパッチファイル作成 | 正常系 |
| PATCH-007 | test_patch_japanese_content | 日本語内容 | 日本語の差分が正しく表示 | 正常系 |
| PATCH-008 | test_patch_binary_skipped | バイナリスキップ | バイナリファイルはパッチ生成されない | 準正常系 |
| PATCH-009 | test_patch_and_patch_file_together | --patch と --patch-file併用 | 両方のパッチファイルが生成 | 正常系 |
| PATCH-010 | test_patch_hunk_header_format | ハンクヘッダー形式 | @@ -start,count +start,count @@ 形式 | 正常系 |

---

### 18. シンボリックリンク不具合修正テスト

**背景**: 統計で「Symlinks: N files」と表示されるのに「Symlink Details」セクションが空になる不具合の修正を検証

| テストID | テスト名 | テスト内容 | 期待結果 | 分類 |
|---------|---------|-----------|---------|------|
| SYMFIX-001 | test_unchanged_symlinks_not_counted | 変更なしシンボリックリンク | 統計でカウントされない、Symlink Details表示なし | 正常系 |
| SYMFIX-002 | test_changed_symlinks_counted_with_details | 変更ありシンボリックリンク | 統計でカウント、Symlink Detailsに詳細表示 | 正常系 |
| SYMFIX-003 | test_symlink_statistics_match_details_count | 統計とDetails一致 | 統計の件数とSymlink Detailsのエントリ数が一致 | 正常系 |
| SYMFIX-004 | test_symlink_info_always_set | symlink_info設定確認 | カウント>0時にSymlink Details必須、"unavailable"なし | 正常系 |
| SYMFIX-005 | test_multiple_unchanged_symlinks_ignored | 複数の変更なしシンボリックリンク | すべて無視、統計0、Details表示なし | 正常系 |
| SYMFIX-006 | test_mixed_changed_unchanged_symlinks | 変更あり/なし混在 | 変更ありのみカウント、変更なしはDetailsに表示なし | 正常系 |

---

## 終了コード一覧

| 終了コード | 意味 |
|-----------|------|
| 0 | 差分あり（正常終了） |
| 1 | エラー発生 |
| 2 | 差分なし |
| 3 | コンフリクトあり（三方向比較時） |

---

## テストデータ構造

```
test_data/
├── source/
│   ├── unchanged.txt      # ターゲットと同一内容
│   ├── modified.txt       # ターゲットと異なる内容
│   ├── deleted.txt        # ターゲットには存在しない
│   └── subdir/
│       └── nested.txt
├── target/
│   ├── unchanged.txt      # ソースと同一内容
│   ├── modified.txt       # ソースと異なる内容
│   ├── added.txt          # ソースには存在しない
│   └── subdir/
│       └── nested.txt
└── 日本語テスト/
    ├── ソース/
    │   └── テストファイル.txt
    └── ターゲット/
        └── テストファイル.txt
```

---

## 実行方法

```bash
# すべての結合テストを実行
cargo test --test integration_tests

# 特定のテストを実行
cargo test --test integration_tests test_two_way_

# 日本語パステストのみ
cargo test --test integration_tests japanese

# 準正常系テストのみ
cargo test --test integration_tests semi_normal

# 詳細出力付きで実行
cargo test --test integration_tests -- --nocapture

# テスト結果を記録
cargo test --test integration_tests 2>&1 | tee test_output.txt
```

---

## 変更履歴

| 日付 | バージョン | 変更内容 |
|------|-----------|---------|
| 2026-01-08 | 1.0 | 初版作成 |
| 2026-01-08 | 1.1 | 準正常系テスト追加、テスト結果ファイル形式定義 |
| 2026-01-08 | 1.2 | 実運用不具合対応テスト追加（エッジケース、設定ファイル、show_unchanged、シンボリックリンク、パーミッション、拡張三方向比較） |
| 2026-01-08 | 1.3 | 出力ファイル内容検証テスト追加（Excel、サマリー、パッチ）、calamineクレート使用 |
| 2026-01-09 | 1.4 | シンボリックリンク不具合修正テスト追加（統計とSymlink Details不一致問題） |
