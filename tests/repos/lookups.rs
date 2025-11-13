use std::time::Instant;

use git_ai::git::{find_repository_in_path, repository::Repository};

use crate::repos::test_repo::TestRepo;

#[test]
fn test_finds_initial_authorship_commit() {
    // Git AI Repo
    let repo = find_repository_in_path(".").unwrap();
    let first_commit_with_authorship = repo.get_first_commit_with_authorship();
    assert!(first_commit_with_authorship.is_some());
}

#[test]
fn tests_does_not_throw_when_no_authorship_commits() {
    // Git AI Repo
    let repo = TestRepo::new();
    let repo = find_repository_in_path(&repo.path().to_str().unwrap())
        .expect("failed to initialize git2 repository");

    let first_commit_with_authorship = repo.get_first_commit_with_authorship();
    assert!(first_commit_with_authorship.is_none());
}
