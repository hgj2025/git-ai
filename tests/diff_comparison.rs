#[macro_use]
mod repos;
use repos::test_repo::TestRepo;
use similar::{ChangeTag, TextDiff};
use std::collections::HashSet;
use std::fs;
use git_ai::git::repository::find_repository_in_path;
use imara_diff::{Algorithm, Diff, InternedInput};

#[test]
fn test_compare_git_diff_vs_textdiff_simple_rewrite() {
    // Create a test repo with the exact scenario from the bug report
    let repo = TestRepo::new();
    let file_path = repo.path().join("Readme.md");

    // First commit: Initial human content
    let initial_content = "## A quick demo of Git AI Rewrites\n\ndasdas\n\nHUMAN";
    fs::write(&file_path, initial_content).unwrap();
    repo.git_ai(&["checkpoint"]).unwrap();
    repo.stage_all_and_commit("Initial README").unwrap();

    // Get the first commit SHA
    let first_commit = repo.git(&["rev-parse", "HEAD"]).unwrap().trim().to_string();

    // Second commit: AI completely rewrites the README
    let new_content = "# Set Operations Library

A TypeScript library providing essential set operations for working with JavaScript `Set` objects. This library offers a collection of utility functions for performing common set operations like union, intersection, difference, and more.

## Features

This library provides the following set operations:

- **Union** - Combine all elements from two sets
- **Intersection** - Find elements common to both sets
- **Difference** - Find elements in the first set but not in the second
- **Symmetric Difference** - Find elements in either set but not in both
- **Superset Check** - Determine if one set contains all elements of another
- **Subset Check** - Determine if one set is contained within another

## Installation

Since this is a TypeScript project, you can use the functions directly by importing them:

```typescript
import { union, intersection, difference } from './set-ops';
// or
import { setUnion, setIntersect, setDiff } from './src/set-ops';
```

## Usage

### Basic Operations

```typescript
import { union, intersection, difference, symmetricDifference } from './set-ops';

// Create some sets
const setA = new Set([1, 2, 3, 4]);
const setB = new Set([3, 4, 5, 6]);

// Union: all elements from both sets
const unionResult = union(setA, setB);
// Result: Set { 1, 2, 3, 4, 5, 6 }

// Intersection: elements in both sets
const intersectionResult = intersection(setA, setB);
// Result: Set { 3, 4 }

// Difference: elements in setA but not in setB
const differenceResult = difference(setA, setB);
// Result: Set { 1, 2 }

// Symmetric Difference: elements in either set but not both
const symDiffResult = symmetricDifference(setA, setB);
// Result: Set { 1, 2, 5, 6 }
```

### Set Relationships

```typescript
import { isSuperset, isSubset } from './set-ops';

const setA = new Set([1, 2, 3, 4, 5]);
const setB = new Set([2, 3, 4]);

// Check if setA is a superset of setB
const isSuper = isSuperset(setA, setB);
// Result: true

// Check if setB is a subset of setA
const isSub = isSubset(setB, setA);
// Result: true
```

### Working with Different Types

All functions are generic and work with any type:

```typescript
// Strings
const fruitsA = new Set(['apple', 'banana', 'orange']);
const fruitsB = new Set(['banana', 'grape', 'apple']);
const allFruits = union(fruitsA, fruitsB);

// Objects (with proper comparison)
const usersA = new Set([{ id: 1 }, { id: 2 }]);
const usersB = new Set([{ id: 2 }, { id: 3 }]);
const allUsers = union(usersA, usersB);
```

## API Reference

### `union<T>(setA: Set<T>, setB: Set<T>): Set<T>`

Returns a new set containing all elements from both `setA` and `setB`.

### `intersection<T>(setA: Set<T>, setB: Set<T>): Set<T>`

Returns a new set containing only the elements that are present in both `setA` and `setB`.

### `difference<T>(setA: Set<T>, setB: Set<T>): Set<T>`

Returns a new set containing elements that are in `setA` but not in `setB`.

### `symmetricDifference<T>(setA: Set<T>, setB: Set<T>): Set<T>`

Returns a new set containing elements that are in either `setA` or `setB`, but not in both.

### `isSuperset<T>(set: Set<T>, subset: Set<T>): boolean`

Returns `true` if `set` contains all elements of `subset`, `false` otherwise.

### `isSubset<T>(set: Set<T>, superset: Set<T>): boolean`

Returns `true` if all elements of `set` are contained in `superset`, `false` otherwise.

## Notes

- All functions return new `Set` objects and do not modify the input sets
- Functions are generic and work with any type `T`
- Empty sets are handled correctly in all operations

## License

This project is open source and available for use.
";

    fs::write(&file_path, new_content).unwrap();
    repo.git_ai(&["checkpoint", "mock_ai", "Readme.md"])
        .unwrap();
    repo.stage_all_and_commit("AI rewrites README").unwrap();

    // Get the second commit SHA
    let second_commit = repo.git(&["rev-parse", "HEAD"]).unwrap().trim().to_string();

    // ========================================================================
    // Compare git diff vs TextDiff
    // ========================================================================

    eprintln!("\n========== COMPARING GIT DIFF VS TEXTDIFF ==========\n");

    // Method 1: Git diff (using git-ai's method)
    let repository = find_repository_in_path(repo.path().to_str().unwrap()).unwrap();
    let git_added_lines = repository
        .diff_added_lines(&first_commit, &second_commit, None)
        .unwrap();

    let git_lines: HashSet<u32> = git_added_lines
        .get("Readme.md")
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .collect();

    eprintln!("Git diff added lines count: {}", git_lines.len());
    eprintln!("Git diff added lines: {:?}\n", {
        let mut sorted: Vec<_> = git_lines.iter().copied().collect();
        sorted.sort();
        sorted
    });

    // Method 2: TextDiff (similar crate)
    let text_diff = TextDiff::from_lines(initial_content, new_content);
    let mut textdiff_lines: HashSet<u32> = HashSet::new();
    let mut current_new_line: u32 = 1;

    for change in text_diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Delete => {
                // Old lines don't increment new line counter
            }
            ChangeTag::Insert => {
                // This is an added line
                textdiff_lines.insert(current_new_line);
                current_new_line += 1;
            }
            ChangeTag::Equal => {
                // Unchanged line
                current_new_line += 1;
            }
        }
    }

    eprintln!("TextDiff added lines count: {}", textdiff_lines.len());
    eprintln!("TextDiff added lines: {:?}\n", {
        let mut sorted: Vec<_> = textdiff_lines.iter().copied().collect();
        sorted.sort();
        sorted
    });

    // Compare the two sets
    let git_only: Vec<u32> = git_lines.difference(&textdiff_lines).copied().collect();
    let textdiff_only: Vec<u32> = textdiff_lines.difference(&git_lines).copied().collect();

    eprintln!("Lines only in git diff: {:?}", git_only);
    eprintln!("Lines only in TextDiff: {:?}", textdiff_only);

    if !git_only.is_empty() || !textdiff_only.is_empty() {
        eprintln!("\n❌ DIFFERENCE DETECTED!");
        eprintln!("Git diff and TextDiff produce different results!");

        // Print detailed diff analysis
        eprintln!("\n========== DETAILED ANALYSIS ==========\n");

        // Show actual line content for discrepancies
        let new_lines: Vec<&str> = new_content.lines().collect();

        if !git_only.is_empty() {
            eprintln!("Lines that git diff says are added, but TextDiff doesn't:");
            for line_num in &git_only {
                let idx = (*line_num as usize).saturating_sub(1);
                if idx < new_lines.len() {
                    eprintln!("  Line {}: {:?}", line_num, new_lines[idx]);
                }
            }
        }

        if !textdiff_only.is_empty() {
            eprintln!("\nLines that TextDiff says are added, but git diff doesn't:");
            for line_num in &textdiff_only {
                let idx = (*line_num as usize).saturating_sub(1);
                if idx < new_lines.len() {
                    eprintln!("  Line {}: {:?}", line_num, new_lines[idx]);
                }
            }
        }

        panic!("Git diff and TextDiff disagree on which lines were added!");
    } else {
        eprintln!("\n✅ Git diff and TextDiff agree on all added lines");
    }
}

#[test]
fn test_compare_diff_algorithms_blank_lines() {
    // Specific test for blank line handling
    let initial = "Line 1\nLine 2\nLine 3";
    let modified = "Line 1\n\nLine 2\n\nLine 3\n\nNew Line";

    eprintln!("\n========== TESTING BLANK LINE HANDLING ==========\n");
    eprintln!("Initial:\n{:?}\n", initial);
    eprintln!("Modified:\n{:?}\n", modified);

    // TextDiff analysis
    let text_diff = TextDiff::from_lines(initial, modified);
    let mut textdiff_added: Vec<u32> = Vec::new();
    let mut current_line: u32 = 1;

    for change in text_diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Delete => {}
            ChangeTag::Insert => {
                textdiff_added.push(current_line);
                eprintln!(
                    "TextDiff Insert line {}: {:?}",
                    current_line,
                    change.value()
                );
                current_line += 1;
            }
            ChangeTag::Equal => {
                current_line += 1;
            }
        }
    }

    eprintln!("\nTextDiff identified these lines as added: {:?}", textdiff_added);
    assert!(!textdiff_added.is_empty(), "TextDiff should detect additions");
}

#[test]
fn test_compare_git_diff_vs_imara_diff_simple_rewrite() {
    // Test using imara-diff with Myers algorithm to see if it matches git diff exactly
    let repo = TestRepo::new();
    let file_path = repo.path().join("Readme.md");

    // First commit: Initial human content
    let initial_content = "## A quick demo of Git AI Rewrites\n\ndasdas\n\nHUMAN";
    fs::write(&file_path, initial_content).unwrap();
    repo.git_ai(&["checkpoint"]).unwrap();
    repo.stage_all_and_commit("Initial README").unwrap();

    // Get the first commit SHA
    let first_commit = repo.git(&["rev-parse", "HEAD"]).unwrap().trim().to_string();

    // Second commit: AI completely rewrites the README
    let new_content = "# Set Operations Library

A TypeScript library providing essential set operations for working with JavaScript `Set` objects. This library offers a collection of utility functions for performing common set operations like union, intersection, difference, and more.

## Features

This library provides the following set operations:

- **Union** - Combine all elements from two sets
- **Intersection** - Find elements common to both sets
- **Difference** - Find elements in the first set but not in the second
- **Symmetric Difference** - Find elements in either set but not in both
- **Superset Check** - Determine if one set contains all elements of another
- **Subset Check** - Determine if one set is contained within another

## Installation

Since this is a TypeScript project, you can use the functions directly by importing them:

```typescript
import { union, intersection, difference } from './set-ops';
// or
import { setUnion, setIntersect, setDiff } from './src/set-ops';
```

## Usage

### Basic Operations

```typescript
import { union, intersection, difference, symmetricDifference } from './set-ops';

// Create some sets
const setA = new Set([1, 2, 3, 4]);
const setB = new Set([3, 4, 5, 6]);

// Union: all elements from both sets
const unionResult = union(setA, setB);
// Result: Set { 1, 2, 3, 4, 5, 6 }

// Intersection: elements in both sets
const intersectionResult = intersection(setA, setB);
// Result: Set { 3, 4 }

// Difference: elements in setA but not in setB
const differenceResult = difference(setA, setB);
// Result: Set { 1, 2 }

// Symmetric Difference: elements in either set but not both
const symDiffResult = symmetricDifference(setA, setB);
// Result: Set { 1, 2, 5, 6 }
```

### Set Relationships

```typescript
import { isSuperset, isSubset } from './set-ops';

const setA = new Set([1, 2, 3, 4, 5]);
const setB = new Set([2, 3, 4]);

// Check if setA is a superset of setB
const isSuper = isSuperset(setA, setB);
// Result: true

// Check if setB is a subset of setA
const isSub = isSubset(setB, setA);
// Result: true
```

### Working with Different Types

All functions are generic and work with any type:

```typescript
// Strings
const fruitsA = new Set(['apple', 'banana', 'orange']);
const fruitsB = new Set(['banana', 'grape', 'apple']);
const allFruits = union(fruitsA, fruitsB);

// Objects (with proper comparison)
const usersA = new Set([{ id: 1 }, { id: 2 }]);
const usersB = new Set([{ id: 2 }, { id: 3 }]);
const allUsers = union(usersA, usersB);
```

## API Reference

### `union<T>(setA: Set<T>, setB: Set<T>): Set<T>`

Returns a new set containing all elements from both `setA` and `setB`.

### `intersection<T>(setA: Set<T>, setB: Set<T>): Set<T>`

Returns a new set containing only the elements that are present in both `setA` and `setB`.

### `difference<T>(setA: Set<T>, setB: Set<T>): Set<T>`

Returns a new set containing elements that are in `setA` but not in `setB`.

### `symmetricDifference<T>(setA: Set<T>, setB: Set<T>): Set<T>`

Returns a new set containing elements that are in either `setA` or `setB`, but not in both.

### `isSuperset<T>(set: Set<T>, subset: Set<T>): boolean`

Returns `true` if `set` contains all elements of `subset`, `false` otherwise.

### `isSubset<T>(set: Set<T>, superset: Set<T>): boolean`

Returns `true` if all elements of `set` are contained in `superset`, `false` otherwise.

## Notes

- All functions return new `Set` objects and do not modify the input sets
- Functions are generic and work with any type `T`
- Empty sets are handled correctly in all operations

## License

This project is open source and available for use.
";

    fs::write(&file_path, new_content).unwrap();
    repo.git_ai(&["checkpoint", "mock_ai", "Readme.md"])
        .unwrap();
    repo.stage_all_and_commit("AI rewrites README").unwrap();

    // Get the second commit SHA
    let second_commit = repo.git(&["rev-parse", "HEAD"]).unwrap().trim().to_string();

    // ========================================================================
    // Compare git diff vs imara-diff (Myers algorithm)
    // ========================================================================

    eprintln!("\n========== COMPARING GIT DIFF VS IMARA-DIFF (Myers) ==========\n");

    // Method 1: Git diff (using git-ai's method)
    let repository = find_repository_in_path(repo.path().to_str().unwrap()).unwrap();
    let git_added_lines = repository
        .diff_added_lines(&first_commit, &second_commit, None)
        .unwrap();

    let git_lines: HashSet<u32> = git_added_lines
        .get("Readme.md")
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .collect();

    eprintln!("Git diff added lines count: {}", git_lines.len());
    eprintln!("Git diff added lines: {:?}\n", {
        let mut sorted: Vec<_> = git_lines.iter().copied().collect();
        sorted.sort();
        sorted
    });

    // Method 2: imara-diff with Myers algorithm (new simplified API)
    let mut imara_lines: HashSet<u32> = HashSet::new();

    // 1. Intern the two strings (default tokenization for &str = lines)
    let input = InternedInput::new(initial_content, new_content);

    // 2. Compute the diff using Myers algorithm
    let mut diff = Diff::compute(Algorithm::Myers, &input);

    // 3. Post-process for git-like behavior
    diff.postprocess_lines(&input);

    // 4. Walk hunks to extract added line numbers
    for hunk in diff.hunks() {
        // Added lines are in the `after` range (1-indexed)
        for line_idx in hunk.after.clone() {
            imara_lines.insert(line_idx + 1); // Convert to 1-indexed
        }
    }

    eprintln!("imara-diff (Myers) added lines count: {}", imara_lines.len());
    eprintln!("imara-diff (Myers) added lines: {:?}\n", {
        let mut sorted: Vec<_> = imara_lines.iter().copied().collect();
        sorted.sort();
        sorted
    });

    // Compare the two sets
    let git_only: Vec<u32> = git_lines.difference(&imara_lines).copied().collect();
    let imara_only: Vec<u32> = imara_lines.difference(&git_lines).copied().collect();

    eprintln!("Lines only in git diff: {:?}", git_only);
    eprintln!("Lines only in imara-diff: {:?}", imara_only);

    if !git_only.is_empty() || !imara_only.is_empty() {
        eprintln!("\n❌ DIFFERENCE DETECTED!");
        eprintln!("Git diff and imara-diff (Myers) produce different results!");

        // Print detailed diff analysis
        eprintln!("\n========== DETAILED ANALYSIS ==========\n");

        // Show actual line content for discrepancies
        let new_lines_vec: Vec<&str> = new_content.lines().collect();

        if !git_only.is_empty() {
            eprintln!("Lines that git diff says are added, but imara-diff doesn't:");
            for line_num in &git_only {
                let idx = (*line_num as usize).saturating_sub(1);
                if idx < new_lines_vec.len() {
                    eprintln!("  Line {}: {:?}", line_num, new_lines_vec[idx]);
                }
            }
        }

        if !imara_only.is_empty() {
            eprintln!("\nLines that imara-diff says are added, but git diff doesn't:");
            for line_num in &imara_only {
                let idx = (*line_num as usize).saturating_sub(1);
                if idx < new_lines_vec.len() {
                    eprintln!("  Line {}: {:?}", line_num, new_lines_vec[idx]);
                }
            }
        }

        panic!("Git diff and imara-diff (Myers) disagree on which lines were added!");
    } else {
        eprintln!("\n✅ Git diff and imara-diff (Myers) agree on all added lines!");
        eprintln!("imara-diff perfectly matches git's behavior!");
    }
}
