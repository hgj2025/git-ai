#[macro_use]
mod repos;
use repos::test_file::ExpectedLineExt;
use repos::test_repo::TestRepo;
use std::fs;

#[test]
fn test_commit_from_subdirectory() {
    // Test that git commit works correctly when run from within a subdirectory
    let repo = TestRepo::new();
    
    // Create a subdirectory structure
    let working_dir = repo.path().join("src").join("lib");
    fs::create_dir_all(&working_dir).unwrap();
    
    // Create initial file in root
    let mut root_file = repo.filename("README.md");
    root_file.set_contents(lines!["# Project".human()]);
    repo.stage_all_and_commit("Initial commit").unwrap();
    
    // Create a file in the subdirectory
    let subdir_file_path = working_dir.join("utils.rs");
    fs::write(&subdir_file_path, "pub fn helper() {\n    println!(\"hello\");\n}\n").unwrap();
    
    // Stage the file
    repo.git(&["add", "src/lib/utils.rs"]).unwrap();
    
    // Create AI checkpoint for the file in subdirectory
    repo.git_ai(&["checkpoint", "mock_ai", "src/lib/utils.rs"]).unwrap();
    
    // Now commit from within the subdirectory (not using -C flag)
    // This simulates running "git commit" from within the subdirectory
    // git-ai should automatically find the repository root
    repo.commit_from_working_dir(&working_dir, "Add utils from subdirectory")
        .expect("Failed to commit from subdirectory");
    
    // Verify that the file was committed and has AI attribution
    let mut file = repo.filename("src/lib/utils.rs");
    file.assert_lines_and_blame(lines![
        "pub fn helper() {".ai(),
        "    println!(\"hello\");".ai(),
        "}".ai(),
    ]);
}

#[test]
fn test_commit_from_subdirectory_with_mixed_files() {
    // Test committing files from both root and subdirectory when commit is run from subdirectory
    let repo = TestRepo::new();
    
    // Create subdirectory structure
    let working_dir = repo.path().join("src");
    fs::create_dir_all(&working_dir).unwrap();
    
    // Create initial commit
    let mut root_file = repo.filename("README.md");
    root_file.set_contents(lines!["# Project".human()]);
    repo.stage_all_and_commit("Initial commit").unwrap();
    
    // Create file in subdirectory (AI-authored)
    let subdir_file_path = working_dir.join("main.rs");
    fs::write(&subdir_file_path, "fn main() {\n    println!(\"Hello, world!\");\n}\n").unwrap();

    // Create AI checkpoint for the file in subdirectory
    repo.git_ai(&["checkpoint", "mock_ai", "src/main.rs"]).unwrap();
    
    // Create file in root (human-authored)
    let root_file_path = repo.path().join("LICENSE");
    fs::write(&root_file_path, "MIT License\n").unwrap();
    
    // Stage both files
    repo.git(&["add", "src/main.rs", "LICENSE"]).unwrap();
    
    // Create human checkpoint
    repo.git_ai(&["checkpoint"]).unwrap(); // Human checkpoint for LICENSE
    
    // Commit from subdirectory (not using -C flag)
    // git-ai should automatically find the repository root
    repo.commit_from_working_dir(&working_dir, "Add files from subdirectory")
        .expect("Failed to commit from subdirectory");
    
    // Verify AI attribution for subdirectory file
    let mut subdir_file = repo.filename("src/main.rs");
    subdir_file.assert_lines_and_blame(lines![
        "fn main() {".ai(),
        "    println!(\"Hello, world!\");".ai(),
        "}".ai(),
    ]);
    
    // Verify human attribution for root file
    let mut license_file = repo.filename("LICENSE");
    license_file.assert_lines_and_blame(lines![
        "MIT License".human(),
    ]);
}

#[test]
fn test_commit_from_nested_subdirectory() {
    // Test committing from a deeply nested subdirectory
    let repo = TestRepo::new();
    
    // Create deeply nested subdirectory structure
    let working_dir = repo.path().join("a").join("b").join("c");
    fs::create_dir_all(&working_dir).unwrap();
    
    // Create initial commit
    let mut root_file = repo.filename("README.md");
    root_file.set_contents(lines!["# Project".human()]);
    repo.stage_all_and_commit("Initial commit").unwrap();
    
    // Create file in nested subdirectory
    let nested_file_path = working_dir.join("deep.rs");
    fs::write(&nested_file_path, "pub mod deep {\n    pub fn func() {}\n}\n").unwrap();
    
    // Stage the file
    repo.git(&["add", "a/b/c/deep.rs"]).unwrap();
    
    // Create AI checkpoint
    repo.git_ai(&["checkpoint", "mock_ai", "a/b/c/deep.rs"]).unwrap();
    
    // Commit from nested subdirectory (not using -C flag)
    // git-ai should automatically find the repository root
    repo.commit_from_working_dir(&working_dir, "Add deep file from nested subdirectory")
        .expect("Failed to commit from nested subdirectory");
    
    // Verify attribution
    let mut file = repo.filename("a/b/c/deep.rs");
    file.assert_lines_and_blame(lines![
        "pub mod deep {".ai(),
        "    pub fn func() {}".ai(),
        "}".ai(),
    ]);
}

