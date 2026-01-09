# rs_diffcopy テスト結果

## 最新テスト実行結果

| 項目 | 内容 |
|-----|------|
| 実施日 | 2026-01-09 |
| 実施時刻 | 03:30 JST |
| 実行環境 | Linux (WSL2) |
| Rustバージョン | stable |
| 結果 | **全テストPASS** |

## テスト結果サマリー

| カテゴリ | テスト数 | PASS | FAIL | スキップ |
|---------|---------|------|------|---------|
| ユニットテスト | 98 | 98 | 0 | 0 |
| 結合テスト | 143 | 143 | 0 | 0 |
| **合計** | **241** | **241** | **0** | **0** |

---

## 結合テスト詳細結果

### 1. 基本動作テスト (4テスト)

| テストID | テスト名 | 結果 | 備考 |
|---------|---------|------|------|
| BASIC-001 | test_help_option | PASS | |
| BASIC-002 | test_help_short_option | PASS | |
| BASIC-003 | test_version_option | PASS | |
| BASIC-004 | test_version_short_option | PASS | |

### 2. 二方向比較テスト (7テスト)

| テストID | テスト名 | 結果 | 備考 |
|---------|---------|------|------|
| TWO-001 | test_added_file_detection | PASS | |
| TWO-002 | test_modified_file_detection | PASS | |
| TWO-003 | test_deleted_file_detection | PASS | |
| TWO-004 | test_unchanged_file_detection | PASS | |
| TWO-005 | test_directory_structure_preserved | PASS | |
| TWO-006 | test_no_differences_exit_code | PASS | |
| TWO-007 | test_differences_exit_code | PASS | |

### 3. 三方向比較テスト (4テスト)

| テストID | テスト名 | 結果 | 備考 |
|---------|---------|------|------|
| THREE-001 | test_three_way_basic | PASS | |
| THREE-002 | test_three_way_conflict | PASS | |
| THREE-003 | test_three_way_both_same_change | PASS | |
| THREE-004 | test_three_way_requires_base | PASS | |

### 4. オプションテスト (11テスト)

| テストID | テスト名 | 結果 | 備考 |
|---------|---------|------|------|
| OPT-001 | test_dry_run | PASS | |
| OPT-002 | test_both_versions | PASS | |
| OPT-003 | test_copy_deleted | PASS | |
| OPT-004 | test_preserve_timestamps | PASS | |
| OPT-005 | test_exclude_pattern | PASS | |
| OPT-006 | test_multiple_exclude_patterns | PASS | |
| OPT-007 | test_patch_generation | PASS | |
| OPT-008 | test_combined_patch_file | PASS | |
| OPT-009 | test_excel_report | PASS | |
| OPT-010 | test_summary_file | PASS | |
| OPT-011 | test_verbose_mode | PASS | |

### 5. 日本語パステスト (11テスト)

| テストID | テスト名 | 結果 | 備考 |
|---------|---------|------|------|
| JP-001 | test_japanese_directory_names | PASS | |
| JP-002 | test_japanese_file_names | PASS | |
| JP-003 | test_japanese_nested_path | PASS | |
| JP-004 | test_mixed_japanese_english_path | PASS | |
| JP-005 | test_japanese_in_summary_output | PASS | |
| JP-006 | test_japanese_both_versions | PASS | |
| JP-007 | test_japanese_copy_deleted | PASS | |
| JP-008 | test_japanese_patch_generation | PASS | |
| JP-009 | test_japanese_excel_report | PASS | |
| JP-010 | test_japanese_summary_file | PASS | |
| JP-011 | test_japanese_three_way | PASS | |

### 6. エラーハンドリングテスト (4テスト)

| テストID | テスト名 | 結果 | 備考 |
|---------|---------|------|------|
| ERR-001 | test_nonexistent_source | PASS | |
| ERR-002 | test_nonexistent_target | PASS | |
| ERR-003 | test_output_exists_without_force | PASS | |
| ERR-004 | test_invalid_glob_pattern | PASS | |

### 7. 準正常系テスト (23テスト)

| テストID | テスト名 | 結果 | 備考 |
|---------|---------|------|------|
| SEMI-001 | test_empty_directories | PASS | |
| SEMI-002 | test_large_file_comparison | PASS | 1MB+ |
| SEMI-003 | test_binary_file_detection | PASS | |
| SEMI-004 | test_symlink_handling | PASS | Unix |
| SEMI-005 | test_special_characters_in_filename | PASS | |
| SEMI-006 | test_deeply_nested_directories | PASS | 12階層 |
| SEMI-007 | test_many_files | PASS | 150ファイル |
| SEMI-008 | test_empty_file | PASS | |
| SEMI-009 | test_three_way_ours_only_change | PASS | |
| SEMI-010 | test_three_way_theirs_only_change | PASS | |
| SEMI-011 | test_three_way_file_added_both | PASS | |
| SEMI-012 | test_three_way_file_deleted_both | PASS | |
| SEMI-013 | test_stats_only | PASS | |
| SEMI-014 | test_filter_status_added | PASS | |
| SEMI-015 | test_filter_status_modified | PASS | |
| SEMI-016 | test_force_option_with_dry_run | PASS | |
| SEMI-017 | test_no_tree_option | PASS | |
| SEMI-018 | test_no_details_option | PASS | |
| SEMI-019 | test_workers_option | PASS | |
| SEMI-020 | test_missing_required_args | PASS | |
| SEMI-021 | test_japanese_content_in_file | PASS | |
| SEMI-022 | test_hiragana_katakana_kanji_mixed | PASS | |
| SEMI-023 | test_long_japanese_filename | PASS | |

### 8. 終了コードテスト (4テスト)

| テストID | テスト名 | 結果 | 備考 |
|---------|---------|------|------|
| EXIT-001 | test_exit_code_0_with_differences | PASS | |
| EXIT-002 | test_exit_code_1_on_error | PASS | |
| EXIT-003 | test_exit_code_2_no_differences | PASS | |
| EXIT-004 | test_exit_code_3_conflicts | PASS | |

### 9. エッジケーステスト (6テスト)

| テストID | テスト名 | 結果 | 備考 |
|---------|---------|------|------|
| EDGE-001 | test_empty_directory_detection | PASS | |
| EDGE-002 | test_empty_directories_both_sides | PASS | |
| EDGE-003 | test_same_size_different_content | PASS | |
| EDGE-004 | test_unicode_russian_filenames | PASS | |
| EDGE-005 | test_exclude_pycache_pattern | PASS | |
| EDGE-006 | test_both_versions_added_files_unchanged | PASS | |

### 10. 設定ファイルテスト (4テスト)

| テストID | テスト名 | 結果 | 備考 |
|---------|---------|------|------|
| CFG-001 | test_config_file_basic | PASS | |
| CFG-002 | test_config_file_with_exclude | PASS | |
| CFG-003 | test_config_file_with_both_versions | PASS | |
| CFG-004 | test_cli_overrides_config | PASS | |

### 11. show_unchangedテスト (3テスト)

| テストID | テスト名 | 結果 | 備考 |
|---------|---------|------|------|
| SHOW-001 | test_show_unchanged_option | PASS | |
| SHOW-002 | test_show_unchanged_short_option | PASS | |
| SHOW-003 | test_unchanged_count_in_statistics | PASS | |

### 12. シンボリックリンクテスト (3テスト - Unix only)

| テストID | テスト名 | 結果 | 備考 |
|---------|---------|------|------|
| SYM-001 | test_symlink_added_detection | PASS | |
| SYM-002 | test_symlink_deleted_detection | PASS | |
| SYM-003 | test_broken_symlink_detection | PASS | |

### 13. パーミッションテスト (2テスト - Unix only)

| テストID | テスト名 | 結果 | 備考 |
|---------|---------|------|------|
| PERM-001 | test_permission_check_scripts_mode | PASS | |
| PERM-002 | test_permission_check_default_disabled | PASS | |

### 14. 拡張三方向比較テスト (9テスト)

| テストID | テスト名 | 結果 | 備考 |
|---------|---------|------|------|
| EXT3-001 | test_three_way_added_ours | PASS | |
| EXT3-002 | test_three_way_added_theirs | PASS | |
| EXT3-003 | test_three_way_deleted_ours | PASS | |
| EXT3-004 | test_three_way_deleted_theirs | PASS | |
| EXT3-005 | test_three_way_deleted_both | PASS | |
| EXT3-006 | test_three_way_modify_delete_conflict | PASS | |
| EXT3-007 | test_three_way_merge_style_ours | PASS | |
| EXT3-008 | test_three_way_merge_style_theirs | PASS | |
| EXT3-009 | test_three_way_conflict_only | PASS | |

### 15. Excelファイル内容検証テスト (6テスト)

| テストID | テスト名 | 結果 | 備考 |
|---------|---------|------|------|
| EXCEL-001 | test_excel_has_correct_sheets | PASS | |
| EXCEL-002 | test_excel_summary_statistics | PASS | |
| EXCEL-003 | test_excel_file_tree_entries | PASS | |
| EXCEL-004 | test_excel_details_sections | PASS | |
| EXCEL-005 | test_excel_japanese_filenames | PASS | |
| EXCEL-006 | test_excel_subdirectory_structure | PASS | |

### 16. サマリーファイル内容検証テスト (8テスト)

| テストID | テスト名 | 結果 | 備考 |
|---------|---------|------|------|
| SUM-001 | test_summary_contains_header | PASS | |
| SUM-002 | test_summary_statistics_accuracy | PASS | |
| SUM-003 | test_summary_contains_file_tree | PASS | |
| SUM-004 | test_summary_contains_details | PASS | |
| SUM-005 | test_summary_japanese_paths | PASS | |
| SUM-006 | test_summary_options_section | PASS | |
| SUM-007 | test_summary_no_differences | PASS | |
| SUM-008 | test_summary_subdirectory_tree | PASS | |

### 17. パッチファイル内容検証テスト (10テスト)

| テストID | テスト名 | 結果 | 備考 |
|---------|---------|------|------|
| PATCH-001 | test_patch_file_unified_format | PASS | |
| PATCH-002 | test_combined_patch_file | PASS | |
| PATCH-003 | test_patch_addition_only | PASS | |
| PATCH-004 | test_patch_deletion_only | PASS | |
| PATCH-005 | test_patch_multiple_hunks | PASS | |
| PATCH-006 | test_patch_subdirectory | PASS | |
| PATCH-007 | test_patch_japanese_content | PASS | |
| PATCH-008 | test_patch_binary_skipped | PASS | |
| PATCH-009 | test_patch_and_patch_file_together | PASS | |
| PATCH-010 | test_patch_hunk_header_format | PASS | |

### 18. シンボリックリンク不具合修正テスト (6テスト)

| テストID | テスト名 | 結果 | 備考 |
|---------|---------|------|------|
| SYMFIX-001 | test_unchanged_symlinks_not_counted | PASS | 変更なしシンボリックリンクがカウントされないことを確認 |
| SYMFIX-002 | test_changed_symlinks_counted_with_details | PASS | 変更ありシンボリックリンクがカウントされ詳細表示されることを確認 |
| SYMFIX-003 | test_symlink_statistics_match_details_count | PASS | 統計のカウントとSymlink Detailsのエントリ数が一致することを確認 |
| SYMFIX-004 | test_symlink_info_always_set | PASS | symlink_infoが適切に設定されることを確認 |
| SYMFIX-005 | test_multiple_unchanged_symlinks_ignored | PASS | 複数の変更なしシンボリックリンクがすべて無視されることを確認 |
| SYMFIX-006 | test_mixed_changed_unchanged_symlinks | PASS | 変更あり/なし混在時に正しくフィルタリングされることを確認 |

### 19. Unchanged/Total統計テスト (6テスト)

| テストID | テスト名 | 結果 | 備考 |
|---------|---------|------|------|
| STATS-001 | test_unchanged_count_always_shown | PASS | --show-unchangedなしでもUnchangedカウントが表示されることを確認 |
| STATS-002 | test_total_equals_all_unique_paths | PASS | Totalが全ユニークパス数（source ∪ target）と一致することを確認 |
| STATS-003 | test_statistics_categories_sum | PASS | 統計カテゴリの合計が妥当であることを確認 |
| STATS-004 | test_unchanged_count_with_many_files | PASS | 多数のファイルがあるときUnchangedカウントが正確であることを確認 |
| STATS-005 | test_unchanged_count_same_with_or_without_option | PASS | --show-unchanged有無でUnchangedカウントが同じであることを確認 |
| STATS-006 | test_unchanged_directories_counted | PASS | 変更なしディレクトリが正しくカウントされることを確認 |

### 20. Excelフォーマット拡張テスト (7テスト)

| テストID | テスト名 | 結果 | 備考 |
|---------|---------|------|------|
| EXFMT-001 | test_excel_summary_has_options_section | PASS | Optionsセクションの存在確認 |
| EXFMT-002 | test_excel_summary_has_statistics_section | PASS | Statisticsセクションの存在確認 |
| EXFMT-003 | test_excel_fold_level_option | PASS | --excel-fold-levelオプション動作確認 |
| EXFMT-004 | test_excel_details_has_header | PASS | Detailsシートヘッダー4列確認 |
| EXFMT-005 | test_excel_summary_has_labels | PASS | Source:, Target:, Output:ラベル確認 |
| EXFMT-006 | test_excel_fold_level_zero_default | PASS | デフォルト（fold-level 0）動作確認 |
| EXFMT-007 | test_excel_file_tree_cell_structure | PASS | ディレクトリ構造がセルで表現されることを確認 |

### 21. Filter statusオプション表示テスト (5テスト)

| テストID | テスト名 | 結果 | 備考 |
|---------|---------|------|------|
| FILT-001 | test_summary_contains_filter_status_option | PASS | Summary Optionsにfilter_status表示確認 |
| FILT-002 | test_excel_contains_filter_status_option | PASS | Excel Optionsにfilter_status表示確認 |
| FILT-003 | test_filter_status_multiple_values | PASS | 複数ステータス（added,modified）表示確認 |
| FILT-004 | test_no_filter_status_when_not_specified | PASS | 未指定時にSummaryにFilter statusなし |
| FILT-005 | test_excel_no_filter_status_when_not_specified | PASS | 未指定時にExcelにFilter statusなし |

---

## ユニットテスト詳細結果 (98テスト)

### src/types.rs (22テスト)

| テスト名 | 結果 |
|---------|------|
| test_file_status_as_str | PASS |
| test_file_status_from_str | PASS |
| test_three_way_status_is_conflict | PASS |
| test_three_way_status_from_str | PASS |
| test_special_file_type_as_str | PASS |
| test_file_entry_new | PASS |
| test_three_way_entry_new | PASS |
| test_comparison_stats_total_changes | PASS |
| test_comparison_stats_has_differences | PASS |
| test_three_way_stats_total_conflicts | PASS |
| test_three_way_stats_has_conflicts | PASS |
| test_three_way_stats_has_differences | PASS |
| test_comparison_result_new | PASS |
| test_three_way_result_new | PASS |
| test_status_filter_empty | PASS |
| test_status_filter_included | PASS |
| test_status_filter_excluded | PASS |
| test_status_filter_three_way | PASS |
| test_check_permissions_mode | PASS |
| test_color_mode | PASS |
| test_log_level | PASS |
| test_merge_style | PASS |

### src/scanner.rs (15テスト)

| テスト名 | 結果 |
|---------|------|
| test_scanner_new_empty_patterns | PASS |
| test_scanner_new_invalid_pattern | PASS |
| test_scanner_scan_empty_directory | PASS |
| test_scanner_scan_basic | PASS |
| test_scanner_scan_identifies_directories | PASS |
| test_scanner_scan_nested_directories | PASS |
| test_scanner_exclusion_glob_patterns | PASS |
| test_scanner_exclusion_multiple_extensions | PASS |
| test_scanner_exclusion_directory_pattern | PASS |
| test_scanner_scan_union_basic | PASS |
| test_scanner_scan_union_with_nonexistent | PASS |
| test_get_symlink_target | PASS |
| test_get_symlink_target_unix | PASS |
| test_is_symlink_broken_regular_file | PASS |
| test_is_symlink_broken_unix | PASS |

### src/utils.rs (14テスト)

| テスト名 | 結果 |
|---------|------|
| test_hash_file | PASS |
| test_hash_file_empty | PASS |
| test_hash_file_nonexistent | PASS |
| test_is_binary_file_text | PASS |
| test_is_binary_file_binary | PASS |
| test_is_binary_file_empty | PASS |
| test_format_size | PASS |
| test_count_directory_contents | PASS |
| test_count_directory_contents_empty | PASS |
| test_count_directory_contents_nonexistent | PASS |
| test_display_width | PASS |
| test_pad_to_width | PASS |
| test_is_script_file | PASS |
| test_get_file_mode | PASS |

### src/safety.rs (14テスト)

| テスト名 | 結果 |
|---------|------|
| test_is_protected_path_unix_root | PASS |
| test_is_protected_path_unix_system_dirs | PASS |
| test_is_protected_path_unix_safe_paths | PASS |
| test_validate_directories_all_exist | PASS |
| test_validate_directories_with_base | PASS |
| test_validate_directories_source_missing | PASS |
| test_validate_directories_target_missing | PASS |
| test_validate_directories_base_missing | PASS |
| test_check_output_directory_not_exists | PASS |
| test_check_output_directory_exists_no_force | PASS |
| test_check_output_directory_exists_dry_run | PASS |
| test_check_output_protected_path | PASS |
| test_home_dir | PASS |
| test_get_protected_paths_not_empty | PASS |

### src/copier.rs (13テスト)

| テスト名 | 結果 |
|---------|------|
| test_copier_new | PASS |
| test_should_copy_added | PASS |
| test_should_copy_modified | PASS |
| test_should_copy_deleted_without_flag | PASS |
| test_should_copy_deleted_with_flag | PASS |
| test_should_copy_unchanged | PASS |
| test_should_copy_with_filter | PASS |
| test_copy_directory | PASS |
| test_copy_file_single | PASS |
| test_copy_file_both_versions | PASS |
| test_copy_file_deleted | PASS |
| test_copy_with_subdirectory | PASS |
| test_preserve_timestamp | PASS |

### src/patch.rs (11テスト)

| テスト名 | 結果 |
|---------|------|
| test_patch_generator_new | PASS |
| test_create_unified_diff_simple | PASS |
| test_create_unified_diff_addition | PASS |
| test_create_unified_diff_deletion | PASS |
| test_create_unified_diff_no_changes | PASS |
| test_create_unified_diff_empty_to_content | PASS |
| test_generate_patch_text_file | PASS |
| test_generate_patch_binary_file | PASS |
| test_write_patch_file | PASS |
| test_write_patch_file_with_subdirectory | PASS |
| test_write_combined_patch | PASS |

### src/excel.rs (9テスト)

| テスト名 | 結果 |
|---------|------|
| test_collect_options_all_enabled | PASS |
| test_collect_options_minimal | PASS |
| test_collect_options_permission_check_all | PASS |
| test_collect_options_filter_status | PASS |
| test_collect_options_filter_status_empty | PASS |
| test_create_formats | PASS |
| test_get_entry_details_modified | PASS |
| test_get_entry_details_error | PASS |
| test_apply_row_grouping | PASS |

---

## テスト実行履歴

| 日付 | 時刻 | Unit | Integration | 結果 | 備考 |
|------|------|------|-------------|------|------|
| 2026-01-08 | 21:40 | 89/89 | 68/68 | PASS | 初回実行、準正常系追加 |
| 2026-01-08 | 22:15 | 89/89 | 95/95 | PASS | 実運用不具合対応テスト追加 |
| 2026-01-08 | 23:30 | 89/89 | 119/119 | PASS | 出力ファイル内容検証テスト追加（Excel/Summary/Patch） |
| 2026-01-09 | 00:30 | 89/89 | 125/125 | PASS | シンボリックリンク不具合修正テスト追加（統計とDetails不一致問題） |
| 2026-01-09 | 01:30 | 89/89 | 131/131 | PASS | Unchanged/Total統計不具合修正テスト追加 |
| 2026-01-09 | 02:30 | 96/96 | 138/138 | PASS | Excelフォーマット拡張テスト追加（罫線、Options、fold-level） |
| 2026-01-09 | 03:30 | 98/98 | 143/143 | PASS | Filter statusオプション表示テスト追加（Summary/Excel） |

---

## テスト実行方法

```bash
# すべてのテストを実行
cargo test

# ユニットテストのみ
cargo test --lib

# 結合テストのみ
cargo test --test integration_tests

# 特定のテストを実行
cargo test --test integration_tests test_japanese

# 詳細出力付き
cargo test -- --nocapture

# テスト結果をファイルに保存
cargo test 2>&1 | tee test_output.txt
```

---

## 注意事項

- 保護ディレクトリテスト（PROT-001〜006）はユニットテスト（src/safety.rs）でのみ実行
- システムディレクトリへの誤操作を防ぐため、結合テストでは保護ディレクトリの実際の削除テストは行わない
- 日本語パステストは UTF-8 対応環境で実行すること
- シンボリックリンクテスト、パーミッションテストはUnix環境でのみ実行

---

## 実運用不具合対応テスト

以下のテストケースは実運用時の不具合対応から追加されたものです：

- **エッジケーステスト**: 空ディレクトリ、同サイズ異内容、ロシア語ファイル名
- **設定ファイルテスト**: TOML設定ファイルの読み込み、CLI引数オーバーライド
- **show_unchangedテスト**: 未変更ファイルの表示オプション
- **シンボリックリンクテスト**: リンク追加・削除・壊れたリンク検出
- **パーミッションテスト**: スクリプトファイルの権限チェック
- **拡張三方向比較テスト**: 追加・削除・modify-delete競合、マージスタイル

## 出力ファイル内容検証テスト

以下のテストは実際に生成されたファイルの内容を読み取り、正確性を検証します：

- **Excelファイル検証** (calamine使用): シート構成、統計値、ファイルエントリ、日本語
- **サマリーファイル検証**: ヘッダー、統計、ファイルツリー、詳細セクション
- **パッチファイル検証**: unified diff形式、ハンク構造、日本語内容、バイナリスキップ

## シンボリックリンク不具合修正テスト

**不具合**: 統計で「Symlinks: N files」と表示されるが「Symlink Details」セクションが空になる問題

**原因**:
1. 変更なしシンボリックリンク（両方で同じリンク先）がカウントされていたが詳細は表示されなかった
2. symlink_infoがNoneの場合、詳細セクションでスキップされていた

**修正内容**:
1. comparator.rs: 変更なしシンボリックリンクはNoneを返すように修正
2. summary.rs: symlink_infoがNoneの場合のフォールバック表示を追加

**テスト内容**:
- 変更なしシンボリックリンクが統計にカウントされないことを検証
- 変更ありシンボリックリンクがカウントされ詳細表示されることを検証
- 統計のカウントとSymlink Detailsのエントリ数が一致することを検証
- 複数の変更なし/あり混在シナリオでの正確なフィルタリングを検証

## Unchanged/Total統計不具合修正テスト

**不具合**: 大量にチェックしているのにUnchangedが0になり、Totalが検査総数にならない問題

**原因**:
1. `compare_file()`メソッドは`show_unchanged`がfalseの場合、変更なしファイルのエントリを返さなかった
2. `calculate_stats()`は`entries.len()`からtotal_itemsを計算しており、変更なしファイルが含まれなかった
3. unchanged_filesカウントもentriesからカウントしていたため、show_unchangedがfalseの場合は0になった

**修正内容**:
1. `comparator.rs`: AtomicUsizeを使用してスレッドセーフにunchangedカウントを追跡
2. 新メソッド`compare_path_with_unchanged()`を作成、`(Option<FileEntry>, bool)`を返すように変更
3. 新メソッド`compare_file_with_unchanged()`を作成、変更なしファイルを識別
4. `calculate_stats()`の引数を`(entries, unchanged_count, total_paths)`に変更
5. `rs_diffcopy.md`: 仕様を明確化
   - Unchanged: 変更なしファイル数（`--show-unchanged`オプションに関わらず常に統計に表示）
   - Total: 検査した全ユニークパス数（source ∪ target）

**テスト内容**:
- `--show-unchanged`オプションなしでもUnchangedカウントが正確に表示されることを検証
- Totalが全ユニークパス数（source ∪ target）と一致することを検証
- 統計カテゴリの合計が妥当であることを検証
- 多数のファイルがある場合のUnchangedカウントの正確性を検証
- `--show-unchanged`有無でUnchangedカウントが同じ値になることを検証
- 変更なしディレクトリが正しくカウントされることを検証

## Excelフォーマット拡張テスト

**不具合**: Excelファイル出力の視認性が低い問題
- セルに罫線がない
- Optionsセクションがない
- Statistics/Detailsのヘッダー幅が不正
- --excel-fold-levelオプションが動作しない

**修正内容**:
1. `excel.rs`: 全データセルに`FormatBorder::Thin`を適用
2. `ExcelFormats`構造体を作成し、共通フォーマットを管理
3. Optionsセクションを追加（使用オプション一覧を表示）
4. Statisticsヘッダーを2列に適用
5. Detailsヘッダーを各列個別に適用（merge_rangeではなく個別設定）
6. `apply_row_grouping()`関数を実装、`worksheet.group_rows()`を使用
7. rust_xlsxwriterを0.79から0.92に更新（group_rows対応）

**テスト内容**:
- SummaryシートにOptionsセクションが存在することを検証
- SummaryシートにStatisticsセクションが存在することを検証
- --excel-fold-levelオプションが正常に動作することを検証
- Detailsシートヘッダーが4列存在することを検証
- Source:, Target:, Output:ラベルが存在することを検証
- File Treeのセル構造が正しいことを検証

## Filter statusオプション表示テスト

**要望**: --filter-statusオプションをOptionsセクションに表示

**修正内容**:
1. `excel.rs`: `collect_options()`に`filter_status`を追加
2. Summaryファイルには既に実装済み（`summary.rs`）

**テスト内容**:
- --filter-status指定時にSummaryファイルのOptionsセクションに表示されることを検証
- --filter-status指定時にExcelのOptionsセクションに表示されることを検証
- 複数ステータス（added,modified等）がカンマ区切りで表示されることを検証
- --filter-status未指定時にFilter status行が表示されないことを検証
