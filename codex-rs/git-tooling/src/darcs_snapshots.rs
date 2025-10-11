use std::ffi::OsStr;
use std::ffi::OsString;
use std::fmt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use codex_core::revision_control::darcs::darcs_cli_available;
use tempfile::Builder;
use tempfile::TempDir;
use walkdir::DirEntry;
use walkdir::WalkDir;

use crate::errors::DarcsSnapshotError;
use crate::errors::SnapshotError;

#[derive(Clone)]
pub(crate) struct DarcsSnapshot {
    id: String,
    relative_path: Option<PathBuf>,
    storage: Arc<TempDir>,
}

impl DarcsSnapshot {
    pub(crate) fn new(id: String, relative_path: Option<PathBuf>, storage: TempDir) -> Self {
        Self {
            id,
            relative_path,
            storage: Arc::new(storage),
        }
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn relative_path(&self) -> Option<&Path> {
        self.relative_path.as_deref()
    }

    pub(crate) fn storage_path(&self) -> &Path {
        self.storage.path()
    }
}

impl fmt::Debug for DarcsSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DarcsSnapshot")
            .field("id", &self.id)
            .field("relative_path", &self.relative_path)
            .field("storage", &self.storage.path())
            .finish()
    }
}

impl PartialEq for DarcsSnapshot {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.relative_path == other.relative_path
            && self.storage_path() == other.storage_path()
    }
}

impl Eq for DarcsSnapshot {}

pub(crate) fn create_snapshot(
    repo_root: &Path,
    scope: &Path,
    storage_root: &Path,
) -> Result<DarcsSnapshot, SnapshotError> {
    if !darcs_cli_available() {
        return Err(DarcsSnapshotError::CliMissing.into());
    }

    let relative = ensure_scope_within_repo(repo_root, scope)?;
    run_darcs_record_dry_run(repo_root)?;

    std::fs::create_dir_all(storage_root).map_err(|source| {
        SnapshotError::from(DarcsSnapshotError::StoragePath {
            path: storage_root.to_path_buf(),
            source,
        })
    })?;

    let tempdir = Builder::new()
        .prefix("codex-darcs-snapshot-")
        .tempdir_in(storage_root)
        .map_err(|err| SnapshotError::from(DarcsSnapshotError::Io(err)))?;

    copy_scope(repo_root, relative.as_deref(), tempdir.path())?;

    let id = tempdir
        .path()
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "codex-darcs-snapshot".to_string());

    Ok(DarcsSnapshot::new(id, relative, tempdir))
}

pub(crate) fn restore_snapshot(
    repo_root: &Path,
    repo_path: &Path,
    snapshot: &DarcsSnapshot,
) -> Result<(), SnapshotError> {
    if !darcs_cli_available() {
        return Err(DarcsSnapshotError::CliMissing.into());
    }

    // Ensure the provided path is inside the repository even though we restore
    // based on the snapshot metadata.
    ensure_scope_within_repo(repo_root, repo_path)?;

    run_darcs_for_status(repo_root, ["revert", "--all"])?;
    clear_target(repo_root, snapshot.relative_path())?;
    copy_snapshot_into_repo(snapshot, repo_root)?;
    Ok(())
}

fn ensure_scope_within_repo(
    repo_root: &Path,
    scope: &Path,
) -> Result<Option<PathBuf>, DarcsSnapshotError> {
    if repo_root == scope {
        return Ok(None);
    }

    if let Ok(relative) = scope.strip_prefix(repo_root) {
        return Ok(non_empty_path(relative));
    }

    let repo_canon = std::fs::canonicalize(repo_root)?;
    let scope_canon = std::fs::canonicalize(scope)?;
    if !scope_canon.starts_with(&repo_canon) {
        return Err(DarcsSnapshotError::PathOutsideRepository {
            path: scope.to_path_buf(),
            root: repo_root.to_path_buf(),
        });
    }

    let relative = scope_canon.strip_prefix(&repo_canon)?;
    Ok(non_empty_path(relative))
}

fn non_empty_path(path: &Path) -> Option<PathBuf> {
    if path.as_os_str().is_empty() {
        None
    } else {
        Some(path.to_path_buf())
    }
}

fn run_darcs_record_dry_run(repo_root: &Path) -> Result<(), DarcsSnapshotError> {
    run_darcs_for_status(
        repo_root,
        [
            "record",
            "--dry-run",
            "--all",
            "--look-for-adds",
            "--patch",
            "codex-snapshot",
            "--author",
            "Codex Snapshot <snapshot@codex.local>",
        ],
    )
}

fn run_darcs_for_status<I, S>(repo_root: &Path, args: I) -> Result<(), DarcsSnapshotError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    use std::process::Command;

    let args_vec: Vec<OsString> = args
        .into_iter()
        .map(|arg| arg.as_ref().to_os_string())
        .collect();
    let output = Command::new("darcs")
        .args(&args_vec)
        .current_dir(repo_root)
        .output()
        .map_err(DarcsSnapshotError::Io)?;

    if output.status.success() {
        Ok(())
    } else {
        Err(DarcsSnapshotError::CommandFailed {
            command: format_command("darcs", &args_vec),
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

fn format_command(program: &str, args: &[OsString]) -> String {
    let mut cmd = String::from(program);
    for arg in args {
        cmd.push(' ');
        cmd.push_str(&arg.to_string_lossy());
    }
    cmd
}

fn copy_scope(
    repo_root: &Path,
    scope: Option<&Path>,
    snapshot_root: &Path,
) -> Result<(), DarcsSnapshotError> {
    let walker = WalkDir::new(repo_root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| should_include_entry(entry, repo_root));

    for entry in walker {
        let entry = entry?;
        let relative = entry.path().strip_prefix(repo_root)?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        if !within_scope(relative, scope) {
            continue;
        }

        let destination = snapshot_root.join(relative);
        copy_entry(&entry, &destination)?;
    }

    Ok(())
}

fn should_include_entry(entry: &DirEntry, repo_root: &Path) -> bool {
    if !entry.file_type().is_dir() {
        return true;
    }

    match entry.path().strip_prefix(repo_root) {
        Ok(relative) => relative
            .components()
            .next()
            .map_or(true, |component| component.as_os_str() != "_darcs"),
        Err(_) => true,
    }
}

fn within_scope(path: &Path, scope: Option<&Path>) -> bool {
    match scope {
        None => true,
        Some(scope_path) => path == scope_path || path.starts_with(scope_path),
    }
}

fn copy_entry(entry: &DirEntry, destination: &Path) -> Result<(), DarcsSnapshotError> {
    let file_type = entry.file_type();
    if file_type.is_dir() {
        std::fs::create_dir_all(destination)?;
        return Ok(());
    }

    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if file_type.is_symlink() {
        copy_symlink(entry.path(), destination)?;
        return Ok(());
    }

    std::fs::copy(entry.path(), destination)?;
    let perms = std::fs::metadata(entry.path())?.permissions();
    std::fs::set_permissions(destination, perms)?;
    Ok(())
}

fn copy_symlink(source: &Path, destination: &Path) -> Result<(), DarcsSnapshotError> {
    let target = std::fs::read_link(source)?;
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&target, destination).map_err(|source_err| {
            DarcsSnapshotError::Symlink {
                target: target.clone(),
                link: destination.to_path_buf(),
                source: source_err,
            }
        })?
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::symlink_dir;
        use std::os::windows::fs::symlink_file;

        let metadata = std::fs::symlink_metadata(source)?;
        let result = if metadata.file_type().is_dir() {
            symlink_dir(&target, destination)
        } else {
            symlink_file(&target, destination)
        };
        result.map_err(|source_err| DarcsSnapshotError::Symlink {
            target: target.clone(),
            link: destination.to_path_buf(),
            source: source_err,
        })?;
    }

    Ok(())
}

fn clear_target(repo_root: &Path, relative: Option<&Path>) -> Result<(), DarcsSnapshotError> {
    match relative {
        None => clear_repository_root(repo_root),
        Some(path) => {
            let target = repo_root.join(path);
            if target.exists() {
                remove_path(&target)?;
            }
            Ok(())
        }
    }
}

fn clear_repository_root(repo_root: &Path) -> Result<(), DarcsSnapshotError> {
    for entry in std::fs::read_dir(repo_root)? {
        let entry = entry?;
        if entry.file_name() == "_darcs" {
            continue;
        }
        remove_path(&entry.path())?;
    }
    Ok(())
}

fn remove_path(path: &Path) -> Result<(), DarcsSnapshotError> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_dir() {
        std::fs::remove_dir_all(path)?;
    } else {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

fn copy_snapshot_into_repo(
    snapshot: &DarcsSnapshot,
    repo_root: &Path,
) -> Result<(), DarcsSnapshotError> {
    let walker = WalkDir::new(snapshot.storage_path()).follow_links(false);
    for entry in walker {
        let entry = entry?;
        let relative = entry.path().strip_prefix(snapshot.storage_path())?;
        if relative.as_os_str().is_empty() {
            continue;
        }

        let destination = repo_root.join(relative);
        copy_entry(&entry, &destination)?;
    }
    Ok(())
}
