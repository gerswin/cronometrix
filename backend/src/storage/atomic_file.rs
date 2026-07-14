use std::fs::{self, File, OpenOptions};
use std::io::Write;
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
        let _ = remove_if_owned(&self.temp_path, self.identity);
        let _ = remove_if_owned(&self.final_path, self.identity);
        if let Some(parent) = self.final_path.parent() {
            let _ = sync_directory(parent);
        }
    }
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
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if file_identity(&metadata)? == identity {
        fs::remove_file(path)
            .with_context(|| format!("remove owned atomic file {}", path.display()))?;
    }
    Ok(())
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

#[cfg(target_os = "linux")]
fn publish_noreplace(source: &Path, destination: &Path) -> std::io::Result<()> {
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

#[cfg(not(target_os = "linux"))]
fn publish_noreplace(source: &Path, destination: &Path) -> std::io::Result<()> {
    // Portable std has no no-replace rename. A same-directory hard link is an
    // atomic no-clobber publication fallback on Unix development platforms;
    // the caller removes the temporary name after syncing the directory.
    fs::hard_link(source, destination)
}
