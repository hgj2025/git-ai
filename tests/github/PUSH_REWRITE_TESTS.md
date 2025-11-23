# Push Hooks Integration Tests

This document describes the integration tests for the push hooks functionality in `src/commands/hooks/push_hooks.rs`.

## Overview

The push hooks automatically inject authorship notes refspecs into git push commands, ensuring that AI authorship metadata is synchronized with code pushes. These tests verify the behavior using real GitHub repositories.

## Test Suite: `push_rewrite.rs`

### Tests Included

1. **`test_push_to_remote_with_no_authorship_notes`**
   - **Purpose**: Verify that notes are automatically pushed even on the first push
   - **Scenario**: Create a repo, push initial commits, verify notes are included
   - **Expected**: Notes exist on remote after push (via automatic hook injection)

2. **`test_first_time_notes_push_uses_force`**
   - **Purpose**: Verify force push behavior when remote has no notes
   - **Scenario**: Manually delete notes from remote, push new commit
   - **Expected**: Hook uses `+refs/notes/ai:refs/notes/ai` (force) to create notes ref

3. **`test_push_to_remote_with_existing_notes_ahead`**
   - **Purpose**: Verify merge behavior when remote has additional notes
   - **Scenario**: Add fake note on remote for non-existent commit, push new commits
   - **Expected**: Notes are merged, not force-pushed (preserves remote notes)

4. **`test_push_to_remote_with_existing_notes_behind`**
   - **Purpose**: Verify fast-forward behavior when local notes are ahead
   - **Scenario**: Create multiple local commits with notes, push to remote
   - **Expected**: Notes are fast-forwarded without force

5. **`test_push_to_fork`**
   - **Purpose**: Verify notes are pushed to fork remotes
   - **Scenario**: Add a fork remote, push to fork
   - **Expected**: Notes are pushed to fork (with force if fork has no notes)

6. **`test_push_with_multiple_refspecs`**
   - **Purpose**: Verify notes are included when pushing multiple branches
   - **Scenario**: Push two branches in a single command
   - **Expected**: Notes refspec is injected alongside branch refspecs

7. **`test_push_with_force_with_lease`**
   - **Purpose**: Verify notes work correctly with force push flags
   - **Scenario**: Amend commit and force push with `--force-with-lease`
   - **Expected**: Notes are still injected and pushed correctly

## Running the Tests

### Run all GitHub integration tests (including push tests):

```bash
./tests/github/scripts/run-github-tests.sh
```

### Run only push-related tests:

```bash
cargo test --test github_integration push_rewrite -- --ignored --nocapture
```

### Run a specific test:

```bash
cargo test --test github_integration test_push_to_fork -- --ignored --nocapture
```

### Keep test repositories for inspection:

```bash
./tests/github/scripts/run-github-tests.sh --no-cleanup
```

Or set the environment variable:

```bash
export GIT_AI_TEST_NO_CLEANUP=1
cargo test --test github_integration push_rewrite -- --ignored --nocapture
```

## Prerequisites

1. **GitHub CLI** must be installed and authenticated:
   ```bash
   gh auth login
   ```

2. **Delete repo scope** may be needed for cleanup:
   ```bash
   gh auth refresh -h github.com -s delete_repo
   ```

3. Tests create real GitHub repositories, so you need:
   - Network access
   - GitHub account
   - Ability to create/delete public repositories

## Implementation Details

### Helper Functions

The test file includes several helper functions:

- `add_manual_note()` - Manually add git notes to simulate various scenarios
- `push_notes_to_remote()` - Push notes with or without force
- `fetch_notes_from_remote()` - Fetch notes from remote for verification
- `notes_exist_on_remote()` - Check if notes exist on remote using `git ls-remote`

### Key Testing Approach

1. **Real GitHub repositories**: Tests use actual GitHub repositories to ensure realistic behavior
2. **Manual note manipulation**: Some tests manually add/remove notes to simulate edge cases
3. **Automatic cleanup**: Test repositories are automatically deleted after tests (unless `--no-cleanup` is used)
4. **Hook integration**: Tests verify that the push hooks properly inject refspecs into real git commands

## Test Coverage

These tests cover the main scenarios from the original requirements:

- ✅ Remote with no authorship notes (force push)
- ✅ Remote with notes that are ahead (merge behavior)
- ✅ Remote with notes that are behind (fast-forward)
- ✅ Pushing to a fork
- ✅ Multiple refspecs in single push
- ✅ Force push with `--force-with-lease`

## Related Files

- **Implementation**: `src/commands/hooks/push_hooks.rs`
- **Unit tests**: `src/commands/hooks/push_hooks.rs` (bottom of file)
- **Test harness**: `tests/github/github_test_harness.rs`
- **Integration tests**: `tests/github/push_rewrite.rs`

## Notes

- Tests are marked with `#[ignore]` so they don't run in normal CI
- Tests require GitHub CLI authentication
- Each test creates a uniquely-named repository with timestamp
- Repository names follow pattern: `git-ai-test-{name}-{timestamp}`
- Tests verify both the existence of notes and their correct synchronization

