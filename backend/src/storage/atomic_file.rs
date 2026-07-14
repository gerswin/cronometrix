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
        let mut published = false;
        let write_result = (|| -> anyhow::Result<()> {
            temp_file
                .write_all(bytes)
                .with_context(|| format!("write atomic temp {}", temp_path.display()))?;
            temp_file
                .sync_all()
                .with_context(|| format!("sync atomic temp {}", temp_path.display()))?;
            drop(temp_file);

            // A hard-link publication is atomic, same-filesystem, and has
            // create-new/no-clobber semantics. Removing the temporary name
            // afterwards leaves the published inode at exactly one path.
            if let Err(error) = fs::hard_link(&temp_path, &final_path) {
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
            fs::remove_file(&temp_path)
                .with_context(|| format!("remove atomic temp {}", temp_path.display()))?;
            sync_directory(parent)?;
            Ok(())
        })();

        if let Err(error) = write_result {
            let _ = fs::remove_file(&temp_path);
            if published {
                let _ = fs::remove_file(&final_path);
            }
            return Err(error);
        }

        Ok(Self {
            temp_path,
            final_path,
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
        let _ = fs::remove_file(&self.temp_path);
        let _ = fs::remove_file(&self.final_path);
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
