# codex-git-tooling

Helpers for interacting with git.

```rust,no_run
use std::path::Path;

use codex_core::revision_control::detect_revision_control;
use codex_git_tooling::{CreateGhostCommitOptions, RepoSnapshotManager};

let repo = Path::new("/path/to/repo");
let revision_control = detect_revision_control(repo).expect("git repository");
let manager = RepoSnapshotManager::new(&revision_control);

// Capture the current working tree as an unreferenced commit.
let ghost = manager.create_snapshot(&CreateGhostCommitOptions::new(repo))?;

// Later, undo back to that state.
manager.restore_snapshot(repo, &ghost)?;
```

Pass a custom message with `.message("…")` or force-include ignored files with
`.force_include(["ignored.log".into()])`.
