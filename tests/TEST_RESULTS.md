# rs_diffcopy テスト結果

## 最新テスト実行結果

| 項目 | 内容 |
|-----|------|
| 実施日 | 2026-01-13 |
| 実施時刻 | 17:40 JST |
| 実行環境 | Linux (WSL2) |
| Rustバージョン | stable |
| 結果 | **全テストPASS** |

## テスト結果サマリー

| カテゴリ | テスト数 | PASS | FAIL | スキップ |
|---------|---------|------|------|---------|
| ユニットテスト | 136 | 136 | 0 | 0 |
| 結合テスト | 209 | 209 | 0 | 0 |
| **合計** | **345** | **345** | **0** | **0** |

## 変更内容（v3.0）

### Treeフォーマット改善

**問題**: Tree出力で最初のレベルのディレクトリ・ファイルに接続線（├── または └──）が表示されず、CompareDirectoryと同じレベルに見えてしまう不具合

**修正前の出力**:
```
CompareDirectory{source, target}
file1.txt                  [modified]
file2.txt                  [added]
```

**修正後の出力**:
```
CompareDirectory{source, target}
├file1.txt                  [modified]
└file2.txt                  [added]
```

**修正内容**:
1. `summary.rs` (L441, L444): `render_tree()` と `calculate_max_path_width()` の呼び出しで `is_root` パラメータを `true` → `false` に変更
2. `three_way_summary.rs` (L198, L221): 同様の修正により、三者間比較でも同じフォーマットに対応

**追加テスト**:
- 結合テスト: 2件追加（TREE-001, TREE-002）
  - `test_first_level_items_have_connectors`: 二者間比較で最初のレベルに接続線が付くことを検証
  - `test_three_way_first_level_items_have_connectors`: 三者間比較で最初のレベルに接続線が付くことを検証
- ユニットテスト: 4件追加（UT-3501 ~ UT-3504）
  - `summary.rs`: `test_tree_format_first_level_connectors`, `test_tree_format_nested_structure`
  - `three_way_summary.rs`: `test_three_way_tree_format_first_level_connectors`, `test_three_way_tree_format_nested_structure`

**検証結果**: 全テストPASS（345件）

---

## 変更内容（v2.8）

### レビュー指摘修正

以下の不具合が修正され、対応するテストが追加されました：

1. **--stats-only のファイル出力への影響**
   - 修正: コンソール出力のみ簡略化、ファイル出力は完全サマリーを出力
   - テスト追加: REV-001, REV-008

2. **--filter-status の別名サポート**
   - 修正: add/a/modify/m/delete/d/same/u/sym/link/spec/perm/err などの別名を正規化
   - テスト追加: REV-002, REV-003, REV-007, UT-3401, UT-3402

3. **--filter-status unchanged の自動有効化**
   - 修正: unchanged指定時に--show-unchangedを自動有効化
   - テスト追加: REV-004

4. **--filter-status all の大文字小文字対応**
   - 修正: ALL/All/all など大文字小文字を区別せず処理
   - テスト追加: REV-005, REV-006

## 変更内容（v2.9）

### Windows コンソール出力修正

**問題**: Windows（PowerShell、cmd.exe）でサマリー出力が途中で切れる不具合

**原因**:
- CP932バイト列を `write_all` で直接stdoutに書き込んでいた
- Windowsコンソールは内部的にUTF-16を期待しているため、CP932罫線バイトが不正なシーケンスとして解釈され出力が途切れていた

**修正内容**:
1. `src/utils.rs` に新しいヘルパー関数を追加
   - `is_console_handle()`: ハンドルがコンソールかどうかを判定
   - `write_console_w()`: WriteConsoleW APIでUTF-16出力

2. 4つの出力関数を更新
   - `println_cp932()`, `print_cp932()`, `eprintln_cp932()`, `eprint_cp932()`
   - コンソール出力: WriteConsoleW でUTF-16出力（罫線が正しく表示）
   - リダイレクト出力: CP932エンコード維持（既存動作保持）

3. 依存関係追加
   - `Cargo.toml` に `winapi` クレートを追加

**検証**:
- 全テスト（339件）通過
- 既存機能に影響なし
- Windows環境での実機確認が必要

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

### 22. Filter status表示・File Tree構造・fold-level不具合修正テスト (5テスト)

| テストID | テスト名 | 結果 | 備考 |
|---------|---------|------|------|
| FILTFIX-001 | test_summary_filter_status_all_with_exclusion | PASS | all,^deletedがSummaryに正しく表示確認 |
| FILTFIX-002 | test_excel_filter_status_all_with_exclusion | PASS | all,^deletedがExcelに正しく表示確認 |
| FILTFIX-003 | test_excel_file_tree_cell_structure | PASS | パスコンポーネントが別々のセルに配置確認 |
| FILTFIX-004 | test_excel_fold_level_groups_correctly | PASS | 深さ≧levelの行がグループ化対象確認 |
| FILTFIX-005 | test_summary_filter_status_only_exclusion | PASS | 除外のみ時に"all (implied)"表示確認 |

### 23. File Treeセル構造・fold-level改善テスト (5テスト)

| テストID | テスト名 | 結果 | 備考 |
|---------|---------|------|------|
| FTREE-001 | test_excel_file_tree_no_cell_repeat | PASS | セル重複省略確認 |
| FTREE-002 | test_excel_fold_level_2_per_directory_grouping | PASS | fold-level 2ディレクトリ単位グループ化確認 |
| FTREE-003 | test_excel_fold_level_3_only_deep_items | PASS | fold-level 3深い項目のみグループ化確認 |
| FTREE-004 | test_excel_directory_boundaries | PASS | ディレクトリ境界検出確認 |
| FTREE-005 | test_excel_file_tree_empty_cells_for_repeated_values | PASS | 同一ディレクトリ内ファイルの空セル確認 |

---

## ユニットテスト詳細結果 (129テスト)

### src/types.rs (32テスト)

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
| test_status_filter_matches_three_way_with_expanded_exclusion | PASS |
| test_status_filter_matches_three_way_with_expanded_inclusion | PASS |
| test_expand_three_way_group_added | PASS |
| test_expand_three_way_group_case_insensitive | PASS |
| test_expand_three_way_group_conflicts | PASS |
| test_expand_three_way_group_deleted | PASS |
| test_expand_three_way_group_modified | PASS |
| test_expand_three_way_group_non_group | PASS |
| test_check_permissions_mode | PASS |
| test_color_mode | PASS |
| test_log_level | PASS |
| test_merge_style | PASS |
| test_status_filter_matches_three_way_with_expanded_exclusion | PASS |
| test_status_filter_matches_three_way_with_expanded_inclusion | PASS |
| test_expand_three_way_group_case_insensitive | PASS |

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

### src/utils.rs (23テスト)

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
| test_display_width_halfwidth_katakana | PASS |
| test_display_width_tree_connectors | PASS |
| test_pad_to_width | PASS |
| test_is_script_file | PASS |
| test_get_file_mode | PASS |
| test_cp932_conversion_ascii | PASS |
| test_cp932_conversion_box_drawing | PASS |
| test_cp932_conversion_japanese | PASS |
| test_cp932_conversion_mixed | PASS |
| test_cp932_conversion_unconvertible | PASS |
| test_eprintln_cp932_basic | PASS |
| test_println_cp932_basic | PASS |

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

### src/excel.rs (20テスト)

| テスト名 | 結果 |
|---------|------|
| test_apply_row_grouping | PASS |
| test_cell_deduplication_logic | PASS |
| test_cell_deduplication_parent_changed | PASS |
| test_collect_options_all_enabled | PASS |
| test_collect_options_filter_status | PASS |
| test_collect_options_filter_status_empty | PASS |
| test_collect_options_minimal | PASS |
| test_collect_options_permission_check_all | PASS |
| test_create_formats | PASS |
| test_format_filter_status_all_only | PASS |
| test_format_filter_status_all_with_exclusion | PASS |
| test_format_filter_status_included_only | PASS |
| test_format_filter_status_only_exclusions | PASS |
| test_get_entry_details_error | PASS |
| test_get_entry_details_modified | PASS |
| test_path_expansion_components | PASS |
| test_row_grouping_expanded_fold_level_2 | PASS |
| test_row_grouping_expanded_fold_level_3 | PASS |
| test_row_info_boundary_detection | PASS |
| test_row_info_depth_calculation | PASS |

### src/summary.rs (3テスト)

| テスト名 | 結果 |
|---------|------|
| test_path_components_extraction | PASS |
| test_path_components_japanese | PASS |
| test_path_components_normalized | PASS |

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
| 2026-01-09 | 05:00 | 102/102 | 148/148 | PASS | Filter status表示・File Tree構造・fold-level不具合修正テスト追加 |
| 2026-01-09 | 06:30 | 109/109 | 153/153 | PASS | File Treeセル構造・fold-level改善テスト追加（セル重複省略、ディレクトリ単位グループ化、境界罫線） |
| 2026-01-09 | 08:00 | 109/109 | 157/157 | PASS | パス展開テスト追加 |
| 2026-01-09 | 12:00 | 109/109 | 163/163 | PASS | 三者間比較Excel修正テスト追加（File Tree形式、filter-status、fold-level） |
| 2026-01-09 | 14:00 | 109/109 | 168/168 | PASS | 三者間比較Excelフォーマット修正テスト追加（Summary罫線、Conflicts/Copied Filesパス分割・ヘッダー幅） |
| 2026-01-09 | 16:00 | 111/111 | 173/173 | PASS | Summaryファイル桁位置揃え機能テスト追加（二者間/三者間、日本語対応、表示幅計算） |
| 2026-01-09 | 20:15 | 119/119 | 178/178 | PASS | 三者間グループキーワード除外テスト追加（^added, ^deleted, ^modified, ^conflicts） |
| 2026-01-12 | 23:30 | 119/119 | 179/179 | PASS | ファイルツリー整列テスト追加（Box Drawing文字表示幅修正、CJK端末対応） |
| 2026-01-12 | 23:45 | 119/119 | 180/180 | PASS | 二者間ネストディレクトリ整列テスト追加、テストヘルパー関数修正 |
| 2026-01-13 | 00:46 | 119/119 | 183/183 | PASS | Section 26に3テスト追加（test_three_way_summary_basic_info_section, test_three_way_conflicts_data_cells, test_three_way_copied_files_data_cells） |
| 2026-01-13 | 02:30 | 122/122 | 188/188 | PASS | ユニットテスト3件追加（summary, types）、結合テスト5件追加（tree_display, three_way_excel_format） |
| 2026-01-13 | 12:15 | 129/129 | 194/194 | PASS | ユニットテスト7件追加（types, copier）、結合テスト6件追加（CP932コンソール出力テスト） |
| 2026-01-13 | 15:30 | 130/130 | 199/199 | PASS | CompareDirectoryルートノード表示テスト3件、CP932 Box Drawing文字テスト2件追加 |

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

## Filter status表示・File Tree構造・fold-level不具合修正テスト

**不具合1**: `--filter-status all,^deleted`がOptionsセクションに正しく表示されない
- `all,^deleted`指定時に「Filter status: 」が空または不正な値になる

**不具合2**: Excel File Treeがセル構造ではなくテキストインデントで表現されている
- 参照ファイル（summay.xlsx）ではディレクトリ階層が別々のセルに配置されている

**不具合3**: `--excel-fold-level`の論理が誤っている
- depth > level ではなく depth >= level でグループ化すべき
- レベル2指定時に第2階層以上がグループ化対象になるべき

**原因**:
1. `format_filter_status()`が`include_all`フラグを考慮していなかった
2. File Treeが単一セルにテキストインデントでパスを書いていた
3. 条件式が`depth > fold_level`になっていた

**修正内容**:
1. `excel.rs`/`summary.rs`: `format_filter_status()`ヘルパー関数を追加
   - `include_all=true`の場合は"all"を先頭に追加
   - 除外のみの場合は"all (implied)"を表示
   - 除外ステータスは"^status"形式で表示
2. `excel.rs`: File Tree書き込みを完全にリライト
   - 各パスコンポーネントを別々の列に配置
   - ディレクトリは末尾に"/"を追加
   - ステータスは最後の列に配置
3. `excel.rs`: `apply_row_grouping()`の条件を修正
   - `depth > fold_level`を`depth >= fold_level`に変更

**テスト内容**:
- `all,^deleted`指定時にSummaryに"Filter status: all, ^deleted"が表示されることを検証
- `all,^deleted`指定時にExcelに"all, ^deleted"が表示されることを検証
- File Treeでパスコンポーネントが別々のセルに配置されることを検証
- fold-level 2指定時に深さ2以上の行がグループ化対象になることを検証
- 除外のみ指定時に"all (implied)"が表示されることを検証

## File Treeセル構造・fold-level改善テスト

**修正要望1**: File Treeのセル重複省略（上のセルと同じ値の場合は記載しない）
- ツリー構造を視覚的に表現するため、同じディレクトリ名は最初の行のみに表示

**修正要望2**: ディレクトリ単位でのfold-levelグループ化
- 連続した行ではなく、同一ディレクトリ配下をまとめてグループ化
- 例: fold-level 2の場合、b/配下とc/配下を別々のグループとして折りたたみ

**修正要望3**: ディレクトリ区切り罫線
- 第1階層が変わるタイミングで下罫線を追加し、ディレクトリ単位を視覚的に区切る

**修正内容**:
1. `excel.rs`: `write_file_tree_sheet()`を完全にリライト
   - セル重複省略ロジック: 前行と同じコンポーネントは空セルで表示
   - `calculate_directory_boundaries()`関数を追加してディレクトリ境界を検出
   - 境界行には太い下罫線を適用
2. `excel.rs`: `apply_row_grouping()`をディレクトリ単位グループ化に修正
   - 同一親ディレクトリを持つアイテムを1つのグループとしてまとめる
   - 異なる親ディレクトリ間でグループを分離

**テスト内容**:
- 親ディレクトリが同じ場合にセルが空になることを検証
- fold-level 2でb/配下とc/配下が別グループとして折りたたまれることを検証
- fold-level 3で深さ3以上の項目のみがグループ化されることを検証
- 第1階層が変わるタイミングでディレクトリ境界が検出されることを検証
- 同一ディレクトリ内の2番目以降のファイルで親ディレクトリセルが空になることを検証

## パス展開テスト

### 24. パス展開テスト (4テスト)

| テストID | テスト名 | 結果 | 備考 |
|---------|---------|------|------|
| PEXP-001 | test_excel_path_expansion_deep_path | PASS | |
| PEXP-002 | test_excel_fold_level_with_expanded_paths | PASS | |
| PEXP-003 | test_excel_intermediate_dirs_empty_status | PASS | |
| PEXP-004 | test_excel_shared_intermediate_dirs | PASS | |

### 25. 三者間比較Excel修正テスト (6テスト)

| テストID | テスト名 | 結果 | 備考 |
|---------|---------|------|------|
| IT-2501 | test_three_way_excel_uses_file_tree_sheet | PASS | File Matrixではなく、File Treeシートが存在することを確認 |
| IT-2502 | test_three_way_filter_status_works | PASS | added-oursのみ表示、conflictは除外されることを確認 |
| IT-2503 | test_three_way_filter_status_in_summary | PASS | SummaryのOptionsセクションにFilter status表示確認 |
| IT-2504 | test_three_way_filter_status_in_excel | PASS | ExcelのSummaryシートにFilter status表示確認 |
| IT-2505 | test_three_way_excel_fold_level | PASS | --excel-fold-level 2で深いファイルがグループ化対象になることを確認 |
| IT-2506 | test_three_way_file_tree_cell_structure | PASS | パスコンポーネントが別々のセルに配置されることを確認 |

### 26. 三者間比較Excelフォーマット修正テスト (8テスト)

| テストID | テスト名 | 結果 | 備考 |
|---------|---------|------|------|
| IT-2601 | test_three_way_summary_has_borders | PASS | Change Matrixセクションが存在し、Unchanged統計があることを確認 |
| IT-2602 | test_three_way_conflicts_has_split_path | PASS | DirectoryとFilenameヘッダーがあり、Pathは存在しないことを確認 |
| IT-2603 | test_three_way_copied_files_has_split_path | PASS | DirectoryとFilenameヘッダーがあり、Pathは存在しないことを確認 |
| IT-2604 | test_three_way_conflicts_header_width | PASS | 9列すべてにヘッダーが存在することを確認 |
| IT-2605 | test_three_way_copied_files_header_width | PASS | 4列すべてにヘッダーが存在することを確認 |
| IT-2606 | test_three_way_summary_basic_info_section | PASS | Base, Ours, Theirsのパス情報が正しく表示されることを確認 |
| IT-2607 | test_three_way_conflicts_data_cells | PASS | コンフリクトファイルのDirectory, Filename, Statusが正しく記載されることを確認 |
| IT-2608 | test_three_way_copied_files_data_cells | PASS | コピー済みファイルのDirectory, Filename, Statusが正しく記載されることを確認 |

### 27. Summaryファイル桁位置揃えテスト (6テスト)

| テストID | テスト名 | 結果 | 備考 |
|---------|---------|------|------|
| IT-2701 | test_two_way_summary_file_alignment | PASS | 異なる長さのパスでもステータスタグが同じ位置に揃うことを確認 |
| IT-2702 | test_two_way_summary_file_alignment_japanese | PASS | 日本語ファイル名でも表示幅でステータスが揃うことを確認 |
| IT-2703 | test_three_way_summary_file_alignment | PASS | 異なる長さのパスでもインジケータが同じ位置に揃うことを確認 |
| IT-2704 | test_three_way_summary_file_alignment_japanese | PASS | 日本語ファイル名でも表示幅でインジケータが揃うことを確認 |
| IT-2705 | test_console_output_not_aligned | PASS | コンソール出力はコンパクト形式（整列なし）のままであることを確認 |
| IT-2706 | test_two_way_file_tree_alignment_nested | PASS | ネストディレクトリでBox Drawing文字を考慮した整列を確認 |

### 28. 三者間グループキーワード除外テスト (5テスト)

| テストID | テスト名 | 結果 | 備考 |
|---------|---------|------|------|
| IT-2801 | test_group_keyword_exclusion_added | PASS | ^addedでadded-*が除外されることを確認 |
| IT-2802 | test_group_keyword_exclusion_deleted | PASS | ^deletedでdeleted-*が除外されることを確認 |
| IT-2803 | test_group_keyword_exclusion_modified | PASS | ^modifiedで変更系ステータスが除外されることを確認 |
| IT-2804 | test_group_keyword_exclusion_conflicts | PASS | ^conflictsでコンフリクト系が除外されることを確認 |
| IT-2805 | test_group_keyword_inclusion_added | PASS | addedでadded-*のみ表示されることを確認 |

### 29. ファイルツリー整列テスト (1テスト)

| テストID | テスト名 | 結果 | 備考 |
|---------|---------|------|------|
| IT-2901 | test_three_way_file_tree_alignment | PASS | Box Drawing文字が幅2で計算され、インジケータが揃うことを確認 |

### 30. Tree表示クロスプラットフォームテスト (5テスト)

| テストID | テスト名 | 結果 | 備考 |
|---------|---------|------|------|
| IT-3001 | test_console_output_contains_box_drawing_chars | PASS | コンソール出力に├, └, ─が含まれることを確認 |
| IT-3002 | test_summary_file_contains_box_drawing_chars | PASS | サマリーファイル出力に├, └, ─, │が含まれることを確認 |
| IT-3003 | test_tree_structure_formatting | PASS | ├── と└── のパターンが正しく使用されることを確認 |
| IT-3004 | test_three_way_tree_contains_box_drawing_chars | PASS | 三者間比較のサマリーにも├, └, ─, │が含まれることを確認 |
| IT-3005 | test_box_drawing_chars_at_different_depths | PASS | ネストしたディレクトリでも正しくBox Drawing文字が使用されることを確認 |

### 31. CP932コンソール出力テスト (6テスト)

| テストID | テスト名 | 結果 | 備考 |
|---------|---------|------|------|
| IT-3101 | test_japanese_console_output_no_crash | PASS | 日本語文字を含むコンソール出力がクラッシュしないことを確認 |
| IT-3102 | test_mixed_japanese_ascii_console_output | PASS | 日本語とASCII混合のコンソール出力が正常に動作することを確認 |
| IT-3103 | test_file_output_utf8_preserved | PASS | ファイル出力がUTF-8で保存されることを確認 |
| IT-3104 | test_three_way_japanese_console_output | PASS | 三者間比較での日本語コンソール出力がクラッシュしないことを確認 |
| IT-3105 | test_verbose_mode_japanese_filenames | PASS | verboseモードで日本語ファイル名が正常に出力されることを確認 |
| IT-3106 | test_error_message_japanese_path | PASS | 日本語パスを含むエラーメッセージが正常に出力されることを確認 |

### 32. CompareDirectoryルートノード表示テスト (3テスト)

| テストID | テスト名 | 結果 | 備考 |
|---------|---------|------|------|
| IT-3201 | test_compare_directory_root_two_way | PASS | 二者間比較でCompareDirectory{source, target}形式で表示されることを確認 |
| IT-3202 | test_compare_directory_root_three_way | PASS | 三者間比較でCompareDirectory{base, ours, theirs}形式で表示されることを確認 |
| IT-3203 | test_compare_directory_uses_basename | PASS | 深いパスでもディレクトリ名のみ（basenamePath）が使用されることを確認 |

### 33. CP932 Box Drawing文字エンコーディングテスト (3テスト)

| テストID | テスト名 | 結果 | 備考 |
|---------|---------|------|------|
| UT-3301 | test_cp932_conversion_box_drawing | PASS | Box Drawing文字がJIS X 0208罫線バイト（0x84XX）に正しく変換されることを確認 |
| IT-3301 | test_cp932_box_drawing_encoding | PASS | Box Drawing文字（├, └, │, ─）がCP932で正しくエンコードされることを確認 |
| IT-3302 | test_cp932_mixed_box_drawing_and_text | PASS | Box Drawing文字と日本語テキストの混合がCP932で正しく処理されることを確認 |

## パス展開機能の不具合修正

**不具合**: 最初のパスが`a/b/c/d/e/f.txt`のように深い場合、`--excel-fold-level 2`指定でも全体が折りたたまれてしまう

**原因**:
1. すべてのパスコンポーネントが1行に書き込まれるため、fold-levelが効かない
2. 例: `|a/|b/|c/|d/|e/|f.txt|added|` が1行で、深さは6だがfold-level 2では全体が折りたたまれる

**修正内容**:
1. `excel.rs`: `write_file_tree_sheet()`を完全にリライト
   - 新しいディレクトリに初めて入る場合、中間ディレクトリを個別行に展開
   - 例: `a/b/c/d/e/f.txt`が最初のパスの場合:
     ```
     行1: |a/|  |  |  |  |      |       |
     行2: |  |b/|  |  |  |      |       |
     行3: |  |  |c/|  |  |      |       |
     行4: |  |  |  |d/|  |      |       |
     行5: |  |  |  |  |e/|      |       |
     行6: |  |  |  |  |  |f.txt |added  |
     ```
   - 中間ディレクトリ行のStatusは空（ファイルではないため）
   - `written_dirs` HashSetで展開済みディレクトリを追跡

2. `excel.rs`: `calculate_boundary_rows()`関数を追加
   - 境界罫線を適用する行を事前計算
   - 書き込み時に適切なフォーマット（罫線あり/なし）を使用
   - 既存の`apply_directory_boundaries()`は内容を上書きする問題があったため削除

3. `excel.rs`: 境界フォーマットを各ステータスに追加
   - `added_format_bottom`, `modified_format_bottom`など
   - 境界行では適切な色とMedium下罫線を組み合わせ

**テスト内容**:
- 深いパス(`a/b/c/d/e/f.txt`)が中間ディレクトリに展開されることを検証
- f.txtが正しい列（column 5）に配置されることを検証
- fold-level 2指定時に展開されたパスが正しくグループ化されることを検証
- 中間ディレクトリ行のStatusが空であることを検証
- 同一ディレクトリ内の複数ファイルが中間行を共有することを検証

## 三者間比較Excel修正テスト

**修正要望**:
1. 三者間比較のExcelレポートで「File Matrix」シートを「File Tree」形式に変更
2. `--excel-fold-level`オプションが三者間モードで動作すること

**不具合修正**:
- `--filter-status`が三者間モードのSummary/Excelのオプションに表示されない
- `--filter-status`が三者間モードでフィルタリングが機能しない

**修正内容**:
1. `three_way_excel.rs`: `write_file_matrix_sheet()`を`write_file_tree_sheet()`に置換
   - 二者間比較と同じセルベースのツリー形式を採用
   - パスコンポーネントを別々の列に配置
   - ステータス固有の色付けフォーマットを適用
   - ディレクトリ境界の罫線を適用
   - 中間ディレクトリ行のパス展開を実装

2. `three_way_excel.rs`: `apply_row_grouping_expanded()`関数を追加
   - ディレクトリ単位でのfold-levelグループ化
   - 二者間比較と同じロジック

3. `three_way_excel.rs`: `write_summary_sheet()`を更新
   - Optionsセクションに`filter_status`を追加
   - `has_options()`メソッドで`filter_status`をチェック

4. `three_way_summary.rs`: Summaryファイルの修正
   - `has_options()`に`filter_status.is_empty()`チェックを追加
   - `generate_options()`に`filter_status`表示を追加

5. `types.rs`: `StatusFilter`に`to_display_string()`メソッドを追加
   - 三者間・二者間共通で使用可能

**テスト内容**:
- 三者間ExcelがFile Matrixではなく、File Treeシートを使用することを検証
- `--filter-status added-ours`指定時に正しくフィルタリングされることを検証
- 三者間SummaryのOptionsセクションにFilter statusが表示されることを検証
- 三者間ExcelのSummaryシートにFilter statusが表示されることを検証
- `--excel-fold-level 2`で深いファイルがグループ化対象になることを検証
- パスコンポーネントが別々のセルに配置されることを検証

## 三者間比較Excelフォーマット修正テスト

**修正要望**:
1. SummaryシートのOptionグループと、Change Matrixグループに罫線を追加
2. Conflictsシートの先頭行の背景色の幅が結果表示の領域にあっていない問題を修正
3. ConflictsシートのPathの行をディレクトリパスとファイル名に分割
4. Copied Filesシートの先頭行の背景色の幅が結果表示の領域にあっていない問題を修正
5. Copied FilesシートのPathの行をディレクトリパスとファイル名に分割

**修正内容**:
1. `three_way_excel.rs`: Summaryシートに罫線フォーマットを追加
   - `border_format`と`label_border_format`を定義（FormatBorder::Thin）
   - Options、Change Matrix各行に罫線を適用

2. `three_way_excel.rs`: Conflictsシートのヘッダーとパス分割
   - ヘッダーを`set_row_format`から個別の`write_string_with_format`に変更
   - 9列すべて（Directory, Filename, Type, Base, Ours, Theirs, Ours Hash, Theirs Hash, Sizes）に背景色適用
   - `Path`列を`Directory`と`Filename`に分割

3. `three_way_excel.rs`: Copied Filesシートのヘッダーとパス分割
   - ヘッダーを`set_row_format`から個別の`write_string_with_format`に変更
   - 4列すべて（Directory, Filename, Status, Source）に背景色適用
   - `Path`列を`Directory`と`Filename`に分割

4. `rs_diffcopy.md`: 要件定義を更新
   - Conflictsシート詳細にDirectory/Filename分割を記載
   - Copied Filesシート詳細にDirectory/Filename分割を記載

**テスト内容**:
- SummaryシートにChange Matrixセクションが存在し、Unchanged統計があることを検証
- ConflictsシートにDirectoryとFilenameヘッダーがあり、Pathは存在しないことを検証
- Copied FilesシートにDirectoryとFilenameヘッダーがあり、Pathは存在しないことを検証
- Conflictsシートのヘッダーに9列すべて存在することを検証
- Copied Filesシートのヘッダーに4列すべて存在することを検証

## Summaryファイル桁位置揃え機能

**修正要望**:
- Summaryファイル出力で、File TreeのステータスやChange Matrixの桁位置をすべてのファイルで揃える
- 日本語を使用するため、半角は1桁、全角は2桁で計算
- 半角カナなども考慮した正確な表示幅計算
- コンソール出力は変更なし（コンパクト形式のまま）

**修正内容**:
1. `summary.rs`: 二者間比較のSummaryファイル出力に整列機能を追加
   - `calculate_max_path_width()`関数を追加して最大パス表示幅を事前計算
   - `render_tree()`に`max_width`パラメータを追加
   - ファイル出力時のみパディングを適用してステータス位置を揃える

2. `three_way_summary.rs`: 三者間比較のSummaryファイル出力を改善
   - 固定40文字幅から動的最大幅に変更
   - `calculate_max_path_width()`関数を追加
   - インジケータ位置を最大パス幅に合わせて揃える

3. `utils.rs`: 表示幅計算のユニットテストを追加
   - 半角カナ（`ｱｲｳ`など）のテスト
   - ツリーコネクタ（`├──`など）のテスト

**テスト内容**:
- 二者間Summaryファイルで異なる長さのパスでもステータスタグが揃うことを検証
- 日本語ファイル名を含む場合も表示幅に基づいて正しく整列されることを検証
- 三者間Summaryファイルでインジケータ位置が揃うことを検証
- コンソール出力がコンパクト形式（整列なし）のままであることを検証

## 三者間グループキーワード除外機能

**修正要望**:
- `--filter-status all,^added`で`added`グループ（added-ours, added-theirs, added-both-same, added-both-diff）を除外したい
- グループキーワードが除外（`^`プレフィックス）でも動作するようにしたい

**修正内容**:
1. `types.rs`: StatusFilterにグループキーワード展開関数を追加
   - `expand_three_way_group()`関数で`added`, `modified`, `deleted`, `conflicts`を展開
   - `matches_three_way()`で大文字小文字を区別しないように修正

2. `cli.rs`: `parse_filter_status()`を更新
   - 元のキーワード（二者間用）と展開後のステータス（三者間用）の両方を挿入

3. `config.rs`: `parse_filter_status_vec()`を更新
   - cli.rsと同様の変更を適用

4. `three_way_summary.rs`: Conflict Detailsセクションにフィルター適用
   - `generate_conflict_details()`呼び出し前に`filter_status.matches_three_way()`でフィルター

5. `three_way_excel.rs`: ConflictsシートにもFilter適用
   - Conflictsシートの項目取得時に`filter_status.matches_three_way()`でフィルター

**テスト内容**:
- `^added`でadded-ours, added-theirs, added-both-same, added-both-diffが除外されることを検証
- `^deleted`でdeleted-ours, deleted-theirs, deleted-bothが除外されることを検証
- `^modified`でours-only, theirs-only, both-same, conflictが除外されることを検証
- `^conflicts`でconflict, added-both-diff, modify-delete, delete-modifyが除外されることを検証
- `added`でadded-*のみが表示されることを検証

## ファイルツリー整列テスト（Box Drawing文字表示幅修正）

**不具合**: Summaryファイル出力のFile Treeで、異なる深さのファイルのインジケータ位置がずれる問題

**例**:
```
│   │   │       └── kas.yml      [○  =  M] theirs-only
│   │   │   └── submit-job.sh    [○  =  M] theirs-only     <-- 13桁から開始（ずれている）
│   │       └── kas-build.sh     [○  =  M] theirs-only
```

**原因**:
1. `unicode_width`クレートがBox Drawing文字（U+2500-U+257F: │, └, ├, ─等）を幅1として返す
2. しかし日本語/CJK端末ではBox Drawing文字は幅2で表示される
3. `render_tree()`の`new_prefix`計算で`is_last=true`の場合に4スペース使用していたが、`│`(幅2)+3スペース=5幅に合わせる必要があった

**修正内容**:
1. `utils.rs`: `display_width()`関数でBox Drawing文字（U+2500-U+257F）を幅2として扱う
   ```rust
   if ('\u{2500}'..='\u{257F}').contains(&c) {
       2
   } else {
       UnicodeWidthChar::width(c).unwrap_or(0)
   }
   ```
2. `three_way_summary.rs`: `new_prefix`計算で`is_last=true`の場合に5スペースを使用
   ```rust
   if is_last {
       format!("{}     ", prefix)  // 5 spaces to match │(2)+3 = 5 width
   } else {
       format!("{}│   ", prefix)   // │(2) + 3 spaces = 5 width
   }
   ```
3. `summary.rs`: 同様の修正

**テスト内容**:
- 異なる深さのディレクトリ構造で、すべてのファイルのインジケータ位置が揃うことを検証
- Box Drawing文字の表示幅が正しく2として計算されることを検証（ユニットテスト）
- `├── `が7幅（├=2 + ─=2 + ─=2 + スペース=1）として計算されることを検証
- `│   `が5幅（│=2 + スペース×3=3）として計算されることを検証

## Windows Tree表示不具合修正（クロスプラットフォームパス処理）

**不具合**: Windows版の場合に、コンソール、サマリーファイルともにTree表示にならない。罫線だけでなくスペースもなく、そもそもTree構造が生成されていない。

**原因**:
- `build_tree()`関数でパスを`'/'`（スラッシュ）で分割していた
- Windowsではパス区切り文字が`'\'`（バックスラッシュ）のため、パスが分割されずTree構造が構築されなかった
- 例: `dir\subdir\file.txt`が`split('/')`で分割されず、1つのエントリとして扱われていた

**修正内容**:
1. `summary.rs`: `build_tree()`を修正
   ```rust
   // 修正前（Unixのみ対応）
   let path_str = entry.relative_path.to_string_lossy().to_string();
   let parts: Vec<&str> = path_str
       .split('/')
       .filter(|s| !s.is_empty())
       .collect();

   // 修正後（クロスプラットフォーム対応）
   let parts: Vec<String> = entry
       .relative_path
       .components()
       .map(|c| c.as_os_str().to_string_lossy().to_string())
       .collect();
   ```

2. `three_way_summary.rs`: 同様の修正

3. `insert_into_tree()`の引数型を`&[&str]` → `&[String]`に変更

**テスト内容**:
- コンソール出力にBox Drawing文字（├, └, ─）が含まれることを検証
- サマリーファイル出力にBox Drawing文字（├, └, ─, │）が含まれることを検証
- Tree構造のフォーマット（├── と└── パターン）が正しいことを検証
- 三者間比較モードでもBox Drawing文字が正しく表示されることを検証
- 異なる深さのディレクトリ構造でBox Drawing文字が正しく使用されることを検証

**ユニットテスト追加**（summary.rs）:
- `test_path_components_extraction`: パスコンポーネントが正しく抽出されることを検証
- `test_path_components_japanese`: 日本語パスが正しく処理されることを検証
- `test_path_components_normalized`: パスの正規化が正しく行われることを検証

## CP932コンソール出力テスト（Windowsコンソール対応）

**目的**: Windows環境でのCP932エンコーディングコンソールにおいて、日本語文字を含む出力がクラッシュしないことを確認

**背景**:
- WindowsのコマンドプロンプトはデフォルトでCP932（Shift_JIS互換）エンコーディングを使用
- UTF-8で出力される日本語文字がCP932に変換できない場合、パニックが発生する可能性がある
- ファイル出力は常にUTF-8で保存されるべき

**テスト内容**:
- `test_japanese_console_output_no_crash`: 日本語ファイル名を含むコンソール出力がクラッシュしないことを検証
- `test_mixed_japanese_ascii_console_output`: 日本語とASCII混合パスの処理が正常に動作することを検証
- `test_file_output_utf8_preserved`: サマリーファイル等の出力がUTF-8で保存されることを検証
- `test_three_way_japanese_console_output`: 三者間比較モードでも日本語パスが正常に処理されることを検証
- `test_verbose_mode_japanese_filenames`: verboseモードでの日本語ファイル名出力が正常であることを検証
- `test_error_message_japanese_path`: 存在しない日本語パスに対するエラーメッセージが正常に出力されることを検証

**実装対応**:
- `utils.rs`: `print_cp932()`/`eprint_cp932()`関数を追加（Windows環境でCP932変換可能な文字のみ出力）
- Linuxでは標準出力をそのまま使用（UTF-8対応）
- ファイル出力（Summary, Excel, Patch）は常にUTF-8を維持
