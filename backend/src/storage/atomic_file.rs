use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context};
use uuid::Uuid;

/// A published file that is removed automatically unless the related database
/// mutation commits and the caller explicitly keeps it.
#[derive(Debug)]
pub struct AtomicFileGuard {
    temp_path: PathBuf,
    final_path: PathBuf,
    identity: FileIdentity,
    kept: bool,
}

impl AtomicFileGuard {
    /// Durably writes `bytes` beneath `root` and atomically publishes them at a
    /// server-generated relative path without replacing an existing file.
    pub fn write(root: &Path, relative_path: &str, bytes: &[u8]) -> anyhow::Result<Self> {
        let relative = Path::new(relative_path);
        if relative_path.is_empty()
            || relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            bail!("atomic file path must be a traversal-free relative path");
        }

        // Validation above intentionally precedes every filesystem operation.
        let final_path = root.join(relative);
        let parent = final_path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("atomic file destination has no parent"))?;
        fs::create_dir_all(parent)
            .with_context(|| format!("create atomic file parent {}", parent.display()))?;

        let (temp_path, mut temp_file) = create_unique_temp(parent)?;
        let identity = file_identity(&temp_file.metadata()?)?;
        let mut published = false;
        let write_result = (|| -> anyhow::Result<()> {
            temp_file
                .write_all(bytes)
                .with_context(|| format!("write atomic temp {}", temp_path.display()))?;
            temp_file
                .sync_all()
                .with_context(|| format!("sync atomic temp {}", temp_path.display()))?;
            drop(temp_file);

            if let Err(error) = publish_noreplace(&temp_path, &final_path) {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    bail!("destination already exists: {}", final_path.display());
                }
                return Err(error).with_context(|| {
                    format!(
                        "publish atomic file {} -> {}",
                        temp_path.display(),
                        final_path.display()
                    )
                });
            }
            published = true;
            sync_directory(parent)?;
            remove_if_owned(&temp_path, identity)?;
            sync_directory(parent)?;
            Ok(())
        })();

        if let Err(error) = write_result {
            let _ = remove_if_owned(&temp_path, identity);
            if published {
                let _ = remove_if_owned(&final_path, identity);
            }
            return Err(error);
        }

        Ok(Self {
            temp_path,
            final_path,
            identity,
            kept: false,
        })
    }

    /// Prevent compensation after the associated database transaction commits.
    pub fn keep(mut self) {
        self.kept = true;
    }
}

impl Drop for AtomicFileGuard {
    fn drop(&mut self) {
        if self.kept {
            return;
        }
        if remove_if_owned(&self.temp_path, self.identity).is_err() {
            warn_cleanup_failure("temporary", self.identity);
        }
        if remove_if_owned(&self.final_path, self.identity).is_err() {
            warn_cleanup_failure("published", self.identity);
        }
        if let Some(parent) = self.final_path.parent() {
            if sync_directory(parent).is_err() {
                warn_cleanup_failure("directory-sync", self.identity);
            }
        }
    }
}

/// Read a regular file that is a direct child of `root` without following a
/// final-component symlink. The opened descriptor is read directly, so a
/// concurrent pathname replacement cannot redirect the read.
pub fn read_owned_file(root: &Path, path: &Path) -> anyhow::Result<Vec<u8>> {
    validate_direct_child(root, path)?;
    let mut file = open_nofollow(path)?;
    if !file.metadata()?.file_type().is_file() {
        bail!("owned path is not a regular file");
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Delete a regular direct child using the same identity-preserving quarantine
/// claim as `AtomicFileGuard` compensation. A concurrent replacement is
/// restored/preserved rather than unlinked.
pub fn remove_owned_file(root: &Path, path: &Path) -> anyhow::Result<()> {
    validate_direct_child(root, path)?;
    let file = match open_nofollow(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    let metadata = file.metadata()?;
    if !metadata.file_type().is_file() {
        bail!("owned path is not a regular file");
    }
    let identity = file_identity(&metadata)?;
    drop(file);
    remove_if_owned(path, identity)
}

fn validate_direct_child(root: &Path, path: &Path) -> anyhow::Result<()> {
    if path.parent() != Some(root)
        || path.extension().and_then(|extension| extension.to_str()) != Some("jpg")
        || path.file_stem().is_none()
    {
        bail!("owned file must be a direct JPEG child of its configured root");
    }
    Ok(())
}

#[cfg(unix)]
fn open_nofollow(path: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;

    #[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd"))]
    const O_NOFOLLOW: i32 = 0x0000_0100;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    const O_NOFOLLOW: i32 = 0x0002_0000;
    #[cfg(not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "linux",
        target_os = "android"
    )))]
    const O_NOFOLLOW: i32 = 0;

    OpenOptions::new()
        .read(true)
        .custom_flags(O_NOFOLLOW)
        .open(path)
}

#[cfg(not(unix))]
fn open_nofollow(path: &Path) -> std::io::Result<File> {
    OpenOptions::new().read(true).open(path)
}

fn create_unique_temp(parent: &Path) -> anyhow::Result<(PathBuf, File)> {
    for _ in 0..16 {
        let temp_path = parent.join(format!(".atomic-{}.tmp", Uuid::new_v4()));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => return Ok((temp_path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("create atomic temp {}", temp_path.display()))
            }
        }
    }
    bail!("could not allocate a unique atomic temp file")
}

fn sync_directory(path: &Path) -> anyhow::Result<()> {
    File::open(path)
        .with_context(|| format!("open atomic parent {}", path.display()))?
        .sync_all()
        .with_context(|| format!("sync atomic parent {}", path.display()))
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileIdentity {
    device: u64,
    inode: u64,
}

#[cfg(unix)]
fn file_identity(metadata: &fs::Metadata) -> anyhow::Result<FileIdentity> {
    use std::os::unix::fs::MetadataExt;

    Ok(FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

#[cfg(unix)]
fn remove_if_owned(path: &Path, identity: FileIdentity) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("atomic file has no parent: {}", path.display()))?;
    let quarantine_id = Uuid::new_v4();
    let quarantine = parent.join(format!(".atomic-quarantine-{quarantine_id}.tmp"));
    match claim_noreplace(path, &quarantine) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "claim atomic file {} as {}",
                    path.display(),
                    quarantine.display()
                )
            })
        }
    }

    let claimed_identity = file_identity(&fs::symlink_metadata(&quarantine)?)?;
    if claimed_identity == identity {
        fs::remove_file(&quarantine)
            .with_context(|| format!("remove owned atomic quarantine {}", quarantine.display()))?;
    } else {
        tracing::warn!(
            quarantine_id = %quarantine_id,
            claimed_device = claimed_identity.device,
            claimed_inode = claimed_identity.inode,
            "atomic cleanup preserved foreign entry; paths omitted"
        );
        if let Err(error) = claim_noreplace(&quarantine, path) {
            // A concurrent writer now owns the public path. Preserve the claimed
            // foreign entry under its unique quarantine name for recovery rather
            // than unlinking either writer's file.
            return Err(error).context("restore foreign atomic entry; retained for recovery");
        }
    }
    Ok(())
}

#[cfg(unix)]
fn warn_cleanup_failure(stage: &'static str, identity: FileIdentity) {
    tracing::warn!(
        stage,
        device = identity.device,
        inode = identity.inode,
        "atomic cleanup failed; paths and content omitted"
    );
}

#[cfg(not(unix))]
#[derive(Clone, Copy, Debug)]
struct FileIdentity;

#[cfg(not(unix))]
fn file_identity(_metadata: &fs::Metadata) -> anyhow::Result<FileIdentity> {
    Ok(FileIdentity)
}

#[cfg(not(unix))]
fn remove_if_owned(_path: &Path, _identity: FileIdentity) -> anyhow::Result<()> {
    // No stable file identity is exposed by portable std on non-Unix targets.
    // Conservatively preserve the path rather than risk deleting a replacement.
    Ok(())
}

#[cfg(not(unix))]
fn warn_cleanup_failure(stage: &'static str, _identity: FileIdentity) {
    tracing::warn!(stage, "atomic cleanup failed; paths and content omitted");
}

#[cfg(target_os = "linux")]
fn rename_noreplace(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::ffi::CString;
    use std::os::raw::{c_char, c_int, c_uint};
    use std::os::unix::ffi::OsStrExt;

    const AT_FDCWD: c_int = -100;
    const RENAME_NOREPLACE: c_uint = 1;

    extern "C" {
        fn renameat2(
            olddirfd: c_int,
            oldpath: *const c_char,
            newdirfd: c_int,
            newpath: *const c_char,
            flags: c_uint,
        ) -> c_int;
    }

    let source = CString::new(source.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "NUL in source path"))?;
    let destination = CString::new(destination.as_os_str().as_bytes()).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "NUL in destination path")
    })?;
    // SAFETY: both C strings are NUL-terminated and live for the duration of
    // the call; AT_FDCWD and RENAME_NOREPLACE are Linux ABI constants.
    let result = unsafe {
        renameat2(
            AT_FDCWD,
            source.as_ptr(),
            AT_FDCWD,
            destination.as_ptr(),
            RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(target_os = "macos")]
fn rename_noreplace(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::ffi::CString;
    use std::os::raw::{c_char, c_int, c_uint};
    use std::os::unix::ffi::OsStrExt;

    const RENAME_EXCL: c_uint = 0x0000_0004;

    extern "C" {
        fn renamex_np(old: *const c_char, new: *const c_char, flags: c_uint) -> c_int;
    }

    let source = CString::new(source.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "NUL in source path"))?;
    let destination = CString::new(destination.as_os_str().as_bytes()).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "NUL in destination path")
    })?;
    // SAFETY: both C strings are NUL-terminated and live for the call;
    // RENAME_EXCL is the Darwin renamex_np no-replace flag.
    let result = unsafe { renamex_np(source.as_ptr(), destination.as_ptr(), RENAME_EXCL) };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn publish_noreplace(source: &Path, destination: &Path) -> std::io::Result<()> {
    rename_noreplace(source, destination)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn claim_noreplace(source: &Path, destination: &Path) -> std::io::Result<()> {
    rename_noreplace(source, destination)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn publish_noreplace(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::hard_link(source, destination)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn claim_noreplace(_source: &Path, _destination: &Path) -> std::io::Result<()> {
    // Safe compensation requires a true atomic no-replace rename. Preserve the
    // public entry on unsupported platforms rather than risk unlinking a swap.
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "atomic no-replace claim is unavailable on this platform",
    ))
}

#[cfg(test)]
mod tests {
    use super::{claim_noreplace, read_owned_file, remove_owned_file};

    #[test]
    fn quarantine_claim_moves_source_without_clobbering() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source");
        let quarantine = tmp.path().join("quarantine");
        std::fs::write(&source, b"owned").unwrap();

        claim_noreplace(&source, &quarantine).unwrap();

        assert!(!source.exists());
        assert_eq!(std::fs::read(&quarantine).unwrap(), b"owned");
    }

    #[test]
    fn quarantine_claim_refuses_existing_destination() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source");
        let quarantine = tmp.path().join("quarantine");
        std::fs::write(&source, b"owned").unwrap();
        std::fs::write(&quarantine, b"foreign").unwrap();

        let error = claim_noreplace(&source, &quarantine).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read(&source).unwrap(), b"owned");
        assert_eq!(std::fs::read(&quarantine).unwrap(), b"foreign");
    }

    #[cfg(unix)]
    #[test]
    fn owned_read_and_delete_refuse_symlink_without_touching_target() {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("target.txt");
        let link = tmp.path().join("capture.jpg");
        std::fs::write(&target, b"secret").unwrap();
        symlink(&target, &link).unwrap();

        assert!(read_owned_file(tmp.path(), &link).is_err());
        assert!(remove_owned_file(tmp.path(), &link).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"secret");
        assert!(std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
    }
}
