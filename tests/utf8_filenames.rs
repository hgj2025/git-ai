/// Tests for UTF-8 filename handling with Chinese characters and emojis.
///
/// This tests verifies that files with non-ASCII characters in their filenames
/// are correctly tracked and attributed when git-ai processes commits.
///
/// Issue: Files with Chinese (or other non-ASCII) characters in filenames were
/// incorrectly classified as human-written because git outputs such filenames
/// with octal escape sequences (e.g., `"\344\270\255\346\226\207.txt"` for "中文.txt").
mod repos;
use git_ai::authorship::stats::CommitStats;
use repos::test_file::ExpectedLineExt;
use repos::test_repo::TestRepo;

/// Extract the first complete JSON object from mixed stdout/stderr output.
fn extract_json_object(output: &str) -> String {
    let start = output.find('{').unwrap_or(0);
    let end = output.rfind('}').unwrap_or(output.len().saturating_sub(1));
    output[start..=end].to_string()
}

#[test]
fn test_chinese_filename_ai_attribution() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file with Chinese characters in the filename
    let mut chinese_file = repo.filename("中文文件.txt");
    chinese_file.set_contents(lines![
        "第一行".ai(),
        "第二行".ai(),
        "第三行".ai(),
    ]);

    // Commit the Chinese-named file
    let commit = repo.stage_all_and_commit("Add Chinese file").unwrap();

    // Verify the authorship log contains the Chinese filename
    assert_eq!(
        commit.authorship_log.attestations.len(),
        1,
        "Should have 1 attestation for the Chinese-named file"
    );
    assert_eq!(
        commit.authorship_log.attestations[0].file_path,
        "中文文件.txt",
        "File path should be the actual UTF-8 filename"
    );

    // Get stats and verify AI attribution is correct
    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    // The key check: ai_additions should NOT be 0
    assert_eq!(
        stats.ai_additions, 3,
        "All 3 lines should be attributed to AI, not human"
    );
    assert_eq!(
        stats.human_additions, 0,
        "No lines should be attributed to human"
    );
    assert_eq!(
        stats.ai_accepted, 3,
        "All 3 AI lines should be counted as accepted"
    );
    assert_eq!(
        stats.git_diff_added_lines, 3,
        "Git should report 3 added lines"
    );
}

#[test]
fn test_emoji_filename_ai_attribution() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file with emoji in the filename
    let mut emoji_file = repo.filename("🚀rocket_launch.txt");
    emoji_file.set_contents(lines![
        "Launch sequence initiated".ai(),
        "Engines igniting".ai(),
        "Liftoff!".ai(),
        "Mission success".ai(),
    ]);

    // Commit the emoji-named file
    let commit = repo.stage_all_and_commit("Add emoji file").unwrap();

    // Verify the authorship log contains the emoji filename
    assert_eq!(
        commit.authorship_log.attestations.len(),
        1,
        "Should have 1 attestation for the emoji-named file"
    );
    assert_eq!(
        commit.authorship_log.attestations[0].file_path,
        "🚀rocket_launch.txt",
        "File path should be the actual UTF-8 filename with emoji"
    );

    // Get stats and verify AI attribution is correct
    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    // The key check: ai_additions should NOT be 0
    assert_eq!(
        stats.ai_additions, 4,
        "All 4 lines should be attributed to AI, not human"
    );
    assert_eq!(
        stats.human_additions, 0,
        "No lines should be attributed to human"
    );
    assert_eq!(
        stats.ai_accepted, 4,
        "All 4 AI lines should be counted as accepted"
    );
    assert_eq!(
        stats.git_diff_added_lines, 4,
        "Git should report 4 added lines"
    );
}

#[test]
fn test_mixed_ascii_and_utf8_filenames() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates multiple files - one with ASCII name, one with Chinese, one with emoji
    let mut ascii_file = repo.filename("normal_file.txt");
    ascii_file.set_contents(lines![
        "Normal line 1".ai(),
        "Normal line 2".ai(),
    ]);

    let mut chinese_file = repo.filename("配置文件.txt");
    chinese_file.set_contents(lines![
        "设置一".ai(),
        "设置二".ai(),
        "设置三".ai(),
    ]);

    let mut emoji_file = repo.filename("🎉celebration.txt");
    emoji_file.set_contents(lines![
        "Party time!".ai(),
    ]);

    // Commit all files together
    let commit = repo.stage_all_and_commit("Add mixed files").unwrap();

    // Verify the authorship log contains all 3 files
    assert_eq!(
        commit.authorship_log.attestations.len(),
        3,
        "Should have 3 attestations for all files"
    );

    // Verify each file path is correctly stored
    let file_paths: Vec<&str> = commit
        .authorship_log
        .attestations
        .iter()
        .map(|a| a.file_path.as_str())
        .collect();
    assert!(
        file_paths.contains(&"normal_file.txt"),
        "Should contain ASCII filename"
    );
    assert!(
        file_paths.contains(&"配置文件.txt"),
        "Should contain Chinese filename"
    );
    assert!(
        file_paths.contains(&"🎉celebration.txt"),
        "Should contain emoji filename"
    );

    // Get stats and verify AI attribution is correct for all files
    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    // Total: 2 + 3 + 1 = 6 AI lines
    assert_eq!(
        stats.ai_additions, 6,
        "All 6 lines should be attributed to AI"
    );
    assert_eq!(
        stats.human_additions, 0,
        "No lines should be attributed to human"
    );
    assert_eq!(
        stats.ai_accepted, 6,
        "All 6 AI lines should be counted as accepted"
    );
    assert_eq!(
        stats.git_diff_added_lines, 6,
        "Git should report 6 added lines"
    );
}

#[test]
fn test_utf8_content_in_file() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file with UTF-8 content (but ASCII filename)
    let mut content_file = repo.filename("content.txt");
    content_file.set_contents(lines![
        "Hello World".ai(),
        "你好世界".ai(),
        "🌍 地球".ai(),
        "مرحبا بالعالم".ai(),
        "Привет мир".ai(),
    ]);

    // Commit the file
    let commit = repo.stage_all_and_commit("Add UTF-8 content").unwrap();

    // Verify the authorship log
    assert_eq!(commit.authorship_log.attestations.len(), 1);

    // Get stats and verify AI attribution is correct
    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    assert_eq!(
        stats.ai_additions, 5,
        "All 5 lines should be attributed to AI"
    );
    assert_eq!(
        stats.human_additions, 0,
        "No lines should be attributed to human"
    );
    assert_eq!(
        stats.ai_accepted, 5,
        "All 5 AI lines should be counted as accepted"
    );
}

#[test]
fn test_utf8_filename_blame() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file with Chinese characters in the filename
    let mut chinese_file = repo.filename("测试文件.rs");
    chinese_file.set_contents(lines![
        "fn main() {".ai(),
        "    println!(\"Hello\");".ai(),
        "}".ai(),
    ]);

    // Commit the Chinese-named file
    repo.stage_all_and_commit("Add test file").unwrap();

    // Verify blame works correctly with the UTF-8 filename
    chinese_file.assert_lines_and_blame(lines![
        "fn main() {".ai(),
        "    println!(\"Hello\");".ai(),
        "}".ai(),
    ]);
}

#[test]
fn test_nested_directory_with_utf8_filename() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file in a nested directory with UTF-8 name
    let mut nested_file = repo.filename("src/模块/组件.ts");
    nested_file.set_contents(lines![
        "export const 组件 = () => {};".ai(),
        "export default 组件;".ai(),
    ]);

    // Commit the file
    let commit = repo.stage_all_and_commit("Add nested UTF-8 file").unwrap();

    // Verify the authorship log contains the correct path
    assert_eq!(commit.authorship_log.attestations.len(), 1);
    assert_eq!(
        commit.authorship_log.attestations[0].file_path,
        "src/模块/组件.ts",
        "File path should preserve UTF-8 in both directory and file names"
    );

    // Get stats and verify AI attribution
    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    assert_eq!(
        stats.ai_additions, 2,
        "Both lines should be attributed to AI"
    );
    assert_eq!(
        stats.human_additions, 0,
        "No lines should be attributed to human"
    );
}

#[test]
fn test_utf8_filename_with_human_and_ai_lines() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // Create a file with mixed human and AI contributions
    let mut mixed_file = repo.filename("数据.json");
    mixed_file.set_contents(lines![
        "{".human(),
        "  \"name\": \"测试\",".ai(),
        "  \"value\": 123,".ai(),
        "  \"enabled\": true".human(),
        "}".human(),
    ]);

    // Commit the file
    repo.stage_all_and_commit("Add data file").unwrap();

    // Get stats and verify attribution
    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    assert_eq!(
        stats.ai_additions, 2,
        "2 lines should be attributed to AI"
    );
    assert_eq!(
        stats.ai_accepted, 2,
        "2 AI lines should be counted as accepted"
    );
    assert_eq!(
        stats.human_additions, 3,
        "3 lines should be attributed to human"
    );
    assert_eq!(
        stats.git_diff_added_lines, 5,
        "Git should report 5 total added lines"
    );
}
