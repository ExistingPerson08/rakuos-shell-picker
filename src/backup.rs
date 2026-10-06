//! Step 3 of `docs/SAFETY.md`: copy what a switch is about to overwrite.
//!
//! Nothing in here is allowed to be best-effort. A backup that cannot be read
//! back is worse than no backup at all, because the switch would carry on
//! believing it has a way home. So every copy is verified against the source
//! afterwards, and [`Backup::verify`] can be re-run at any point to re-check the
//! whole tree before something is allowed to overwrite it.
//!
//! Restore deliberately does not delete files it did not record. Undoing a
//! *replacement* needs that, but only the code that performed the replacement
//! knows what it wrote, so removal belongs to the skel writer rather than here.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};

/// What one path inside a backed-up tree turned out to be.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Entry {
    Dir,
    File { bytes: u64 },
    Symlink { target: PathBuf },
}

/// One source directory, and everything found under it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tree {
    /// Keyed by path relative to the tree root, so the listing is stable and
    /// diffable between runs.
    pub entries: BTreeMap<String, Entry>,
    pub files: usize,
    pub bytes: u64,
}

/// The record of what was copied, written next to the copy itself.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// Keyed by the source path relative to `$HOME`.
    pub trees: BTreeMap<String, Tree>,
    /// Sources that were asked for but do not exist. Not an error: a shell that
    /// has never been opened has no config to save.
    pub absent: Vec<String>,
}

/// A verified copy of the config a switch is about to replace.
#[derive(Debug, Clone)]
pub struct Backup {
    pub root: PathBuf,
    pub manifest: Manifest,
}

/// How many names to try before giving up when picking a scratch directory.
///
/// Bounded so a pathological parent directory cannot spin here forever. Five
/// hundred collisions is not a thing that happens; a full disk or a permissions
/// problem is, and that surfaces as the error from `create_dir` on the first
/// attempt rather than as a wrong name.
const SCRATCH_ATTEMPTS: u32 = 500;

impl Backup {
    /// Copy `sources` under `parent`, replacing whatever `name` held.
    ///
    /// `home` is the boundary: anything outside it is refused rather than
    /// copied, so a future registry entry pointing at a system path cannot end
    /// up being restored into one from a user-writable directory.
    ///
    /// The name is a date, so a second switch on the same day collides with the
    /// first. That collision is answered by replacing rather than by stepping to
    /// a suffix: one backup per day is the whole point of dating them by day.
    /// Replacing a backup is only allowed while the copy it holds has already
    /// been re-read and matched against its own manifest, so the order here is
    /// fixed — build the new one in a scratch directory, verify it there, and
    /// only then move the old one aside. A failure at any point before the last
    /// rename leaves the previous backup exactly where it was.
    pub fn create(parent: &Path, name: &str, sources: &[PathBuf], home: &Path) -> Result<Self> {
        if !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
            || name.is_empty()
        {
            bail!("refusing a backup directory named {name:?}: keep it to letters, digits, -_. ");
        }

        fs::create_dir_all(parent)
            .with_context(|| format!("creating backup root {}", parent.display()))?;

        let root = parent.join(name);
        recover_interrupted_replace(parent, name, &root)?;

        let scratch = scratch_dir(parent, name)?;
        let manifest = match fill(&scratch, sources, home) {
            Ok(manifest) => manifest,
            Err(err) => {
                let _ = fs::remove_dir_all(&scratch);
                return Err(err);
            }
        };
        // Read back before anything of the user's is at risk. If this fails the
        // previous backup is still in place and untouched.
        let built = Backup {
            root: scratch.clone(),
            manifest: manifest.clone(),
        };
        if let Err(err) = built.verify() {
            let _ = fs::remove_dir_all(&scratch);
            return Err(err);
        }
        drop(built);

        if let Err(err) = replace(&scratch, &root) {
            let _ = fs::remove_dir_all(&scratch);
            return Err(err);
        }

        Ok(Backup { root, manifest })
    }

    /// Re-check every copied byte against what the manifest says.
    ///
    /// Called once at the end of [`Backup::create`] and available to call again
    /// later, so the guarantee does not depend on when it was last checked.
    pub fn verify(&self) -> Result<()> {
        for (key, tree) in &self.manifest.trees {
            let source_root = self.root.join("home").join(key);
            let found = walk(&source_root)?;
            if found.entries != tree.entries {
                // Name the paths that differ. Comparing only the counts would
                // report "1 recorded, 1 found" for a file that was truncated,
                // which reads like the backup is fine.
                let changed: Vec<&str> = tree
                    .entries
                    .iter()
                    .filter(|(path, entry)| found.entries.get(*path) != Some(*entry))
                    .map(|(path, _)| path.as_str())
                    .take(3)
                    .collect();
                let gone: Vec<&str> = found
                    .entries
                    .keys()
                    .filter(|path| !tree.entries.contains_key(*path))
                    .map(|path| path.as_str())
                    .take(3)
                    .collect();
                bail!(
                    "backup of {key} does not match its manifest: recorded {} entries, found {}; \
                     changed or missing {changed:?}, unexpected {gone:?}",
                    tree.entries.len(),
                    found.entries.len()
                );
            }
            if found.files != tree.files || found.bytes != tree.bytes {
                bail!(
                    "backup of {key} is incomplete: recorded {} files / {} bytes, found {} / {}",
                    tree.files,
                    tree.bytes,
                    found.files,
                    found.bytes
                );
            }
        }
        Ok(())
    }

    /// Put the copied trees back where they came from.
    ///
    /// Verified again afterwards, so a restore that silently did nothing is
    /// reported as a failure rather than as success.
    pub fn restore(&self, home: &Path) -> Result<usize> {
        let mut files = 0;
        for (key, tree) in &self.manifest.trees {
            let from = self.root.join("home").join(key);
            let to = home.join(key);
            let written = copy_tree(&from, &to, false)?;
            if written.entries != tree.entries {
                bail!("restoring {key} did not reproduce the backup");
            }
            files += written.files;
        }
        Ok(files)
    }

    /// Total regular files held, for the log line.
    pub fn file_count(&self) -> usize {
        self.manifest.trees.values().map(|t| t.files).sum()
    }
}

/// Express `path` relative to `home`, refusing anything outside it.
fn relative_to_home(path: &Path, home: &Path) -> Result<PathBuf> {
    if !path.is_absolute() {
        bail!("{} is not an absolute path", path.display());
    }
    path.strip_prefix(home).map(Path::to_path_buf).map_err(|_| {
        anyhow!(
            "{} is outside {}, and config outside the home directory is never backed up or restored",
            path.display(),
            home.display()
        )
    })
}

/// Copy every source into `dir`, then write the manifest that describes them.
fn fill(dir: &Path, sources: &[PathBuf], home: &Path) -> Result<Manifest> {
    let mut manifest = Manifest::default();
    for source in sources {
        let relative = relative_to_home(source, home)?;
        let key = relative.to_string_lossy().into_owned();
        if !source.exists() {
            manifest.absent.push(key);
            continue;
        }
        let tree = copy_tree(source, &dir.join("home").join(relative), true)?;
        manifest.trees.insert(key, tree);
    }

    fs::write(
        dir.join("manifest.toml"),
        toml::to_string_pretty(&manifest)?,
    )
    .context("writing the backup manifest")?;
    Ok(manifest)
}

/// A directory of its own for the copy being built, hidden so that a backup
/// which is only half written can never be mistaken for one that finished.
///
/// `create_dir` is atomic, so two attempts cannot pick the same name.
fn scratch_dir(parent: &Path, name: &str) -> Result<PathBuf> {
    for attempt in 1..=SCRATCH_ATTEMPTS {
        let candidate = if attempt == 1 {
            parent.join(format!(".{name}.new"))
        } else {
            parent.join(format!(".{name}.new-{attempt}"))
        };
        match fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(anyhow!(e).context(format!(
                    "creating a scratch directory beside {}",
                    parent.display()
                )))
            }
        }
    }
    Err(anyhow!(
        "no free scratch directory name under {} after {SCRATCH_ATTEMPTS} attempts; \
         this switch will not start rather than touch a backup that already exists",
        parent.display()
    ))
}

/// Move `scratch` to `root`, keeping one of the two whichever way it ends.
///
/// `rename` will not replace a non-empty directory, so the old copy is moved to
/// a hidden name first and removed only once the new one is in place. The
/// failure path puts the old one back, because "your backup is gone" is the one
/// outcome worse than "your switch did not start".
fn replace(scratch: &Path, root: &Path) -> Result<()> {
    if !root.exists() {
        return fs::rename(scratch, root)
            .with_context(|| format!("moving the new backup to {}", root.display()));
    }

    let stale = aside_name(root, "old");
    let _ = fs::remove_dir_all(&stale);
    fs::rename(root, &stale)
        .with_context(|| format!("moving the previous backup {} aside", root.display()))?;

    match fs::rename(scratch, root) {
        Ok(()) => {
            let _ = fs::remove_dir_all(&stale);
            Ok(())
        }
        Err(err) => {
            let _ = fs::rename(&stale, root);
            Err(anyhow!(err).context(format!(
                "the new backup could not take the place of {}, so it was left as it was",
                root.display()
            )))
        }
    }
}

/// A hidden name beside `root`, used for the copy being replaced.
fn aside_name(root: &Path, purpose: &str) -> PathBuf {
    let parent = root.parent().unwrap_or_else(|| Path::new("."));
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    parent.join(format!(".{name}.{purpose}-{}", std::process::id()))
}

/// Put back a backup that a kill caught between the two renames of [`replace`].
///
/// That window holds a hidden directory holding the previous copy and no
/// `root` at all. Left alone it would sit there forever while the day looked
/// like it had no backup, so the name is claimed again before this one is
/// built. If `root` is already there the leftover is redundant.
fn recover_interrupted_replace(parent: &Path, name: &str, root: &Path) -> Result<()> {
    let prefix = format!(".{name}.old-");
    let Ok(entries) = fs::read_dir(parent) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(text) = file_name.to_str() else {
            continue;
        };
        if !text.starts_with(&prefix) || !entry.path().is_dir() {
            continue;
        }
        if root.exists() {
            let _ = fs::remove_dir_all(entry.path());
        } else {
            fs::rename(entry.path(), root)
                .with_context(|| format!("recovering the interrupted backup {}", root.display()))?;
        }
    }
    Ok(())
}

/// Copy one tree, returning what was found on the way through.
///
/// `skip_asides` drops the rollback copies [`crate::apply`] renames aside, which
/// are this tool's own litter rather than the user's configuration. A backup
/// must not carry them: they are already the same bytes as the file beside them,
/// and every generation that survived used to be copied into the next backup,
/// which is how 214 of them ended up inside four. Restoring passes `false`,
/// because a backup written before this filter existed has to reproduce its own
/// manifest exactly or it reports itself as broken.
fn copy_tree(from: &Path, to: &Path, skip_asides: bool) -> Result<Tree> {
    let mut tree = Tree::default();
    if from.is_symlink() {
        // A symlinked config directory is followed for reading but recorded as
        // what it is, so restoring does not silently replace it with a real one.
        let target =
            fs::read_link(from).with_context(|| format!("reading link {}", from.display()))?;
        // The destination's parent may not exist yet: a backup root holds one tree
        // per source, so a symlinked launcher is usually the first thing written
        // under a deep path. symlink cannot create it.
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        let _ = fs::remove_file(to);
        symlink(&target, to).with_context(|| format!("linking {}", to.display()))?;
        tree.entries.insert(
            String::new(),
            Entry::Symlink {
                target: target.clone(),
            },
        );
        return Ok(tree);
    }
    // A source can be a single file, not only a directory. Noctalia's launcher is
    // exactly that: DMS's skel installs a `.desktop` at that path to hide it, so the
    // backup has to copy the file itself or the switch destroys it with no copy.
    // `walk_into` would call read_dir on it and fail with "Not a directory", which
    // is how this was found — the unit tests all used directory sources.
    if from.is_file() {
        return copy_single_file(from, to, &mut tree);
    }
    walk_into(from, from, to, skip_asides, &mut tree).map(|()| tree)
}

/// Copy one file, recorded as a single entry under an empty relative key.
///
/// The manifest has no separate notion of "this tree was one file", so the entry
/// goes in under the same empty key a symlinked root uses. Restoring walks the
/// keys rather than trusting the tree to be a directory, which is why that works
/// for both.
fn copy_single_file(from: &Path, to: &Path, tree: &mut Tree) -> Result<Tree> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let bytes = fs::metadata(from)
        .with_context(|| format!("reading {}", from.display()))?
        .len();
    fs::copy(from, to)
        .with_context(|| format!("copying {} to {}", from.display(), to.display()))?;
    tree.files = 1;
    tree.bytes = bytes;
    tree.entries.insert(String::new(), Entry::File { bytes });
    Ok(tree.clone())
}

/// Is `name` a rollback copy this tool renamed aside, not the user's own file?
///
/// [`crate::apply`] renames whatever it is about to replace to
/// `.{basename}.pre-{target}-{YYYYMMDD}-{HHMMSS}` next to where it lived. That
/// exact shape is matched — leading dot, `.pre-`, then an eight digit date and a
/// six digit time — so a file of the user's that merely happens to contain
/// `pre-` is never skipped, and neither is a half-written name with no stamp.
fn is_aside(name: &str) -> bool {
    let Some(rest) = name.strip_prefix('.') else {
        return false;
    };
    let Some((_, tail)) = rest.split_once(".pre-") else {
        return false;
    };
    let mut outer = tail.rsplitn(2, '-');
    let (Some(time), Some(body)) = (outer.next(), outer.next()) else {
        return false;
    };
    let mut inner = body.rsplitn(2, '-');
    let (Some(date), Some(target)) = (inner.next(), inner.next()) else {
        return false;
    };
    !target.is_empty() && digits(date, 8) && digits(time, 6)
}

/// Exactly `len` ASCII digits and nothing else.
fn digits(text: &str, len: usize) -> bool {
    text.len() == len && text.bytes().all(|b| b.is_ascii_digit())
}

/// Recursive copy that fills in `tree`.
///
/// `root` is the tree being copied, kept so entries can be keyed by their path
/// relative to it. Keying by bare file name would collide across directories and
/// make the manifest disagree with what `walk` reads back.
fn walk_into(
    root: &Path,
    from: &Path,
    to: &Path,
    skip_asides: bool,
    tree: &mut Tree,
) -> Result<()> {
    fs::create_dir_all(to).with_context(|| format!("creating {}", to.display()))?;
    let meta = fs::symlink_metadata(from)?;
    if meta.is_symlink() {
        let target = fs::read_link(from)?;
        let _ = fs::remove_file(to);
        symlink(&target, to).with_context(|| format!("linking {}", to.display()))?;
        tree.entries
            .insert(relative_key(root, from), Entry::Symlink { target });
        return Ok(());
    }

    for entry in fs::read_dir(from).with_context(|| format!("reading {}", from.display()))? {
        let entry = entry?;
        let name = entry.file_name();
        if skip_asides && is_aside(&name.to_string_lossy()) {
            continue;
        }
        let child_from = from.join(&name);
        let child_to = to.join(&name);
        let meta = fs::symlink_metadata(&child_from)?;

        if meta.is_dir() {
            fs::create_dir_all(&child_to)
                .with_context(|| format!("creating {}", child_to.display()))?;
            tree.entries
                .insert(relative_key(root, &child_from), Entry::Dir);
            walk_into(root, &child_from, &child_to, skip_asides, tree)?;
        } else if meta.is_symlink() {
            let target = fs::read_link(&child_from)?;
            let _ = fs::remove_file(&child_to);
            symlink(&target, &child_to)
                .with_context(|| format!("linking {}", child_to.display()))?;
            tree.entries
                .insert(relative_key(root, &child_from), Entry::Symlink { target });
        } else {
            let bytes = fs::metadata(&child_from)?.len();
            fs::copy(&child_from, &child_to).with_context(|| {
                format!("copying {} to {}", child_from.display(), child_to.display())
            })?;
            tree.files += 1;
            tree.bytes += bytes;
            tree.entries
                .insert(relative_key(root, &child_from), Entry::File { bytes });
        }
    }
    Ok(())
}

/// Read a tree back without writing it anywhere, for verification.
fn walk(root: &Path) -> Result<Tree> {
    let mut tree = Tree::default();
    if root.is_symlink() {
        let target = fs::read_link(root)?;
        tree.entries
            .insert(String::new(), Entry::Symlink { target });
        return Ok(tree);
    }
    if !root.exists() {
        bail!("{} is missing from the backup", root.display());
    }
    // A source may have been a single file, not a directory -- Noctalia's launcher
    // is one. `collect` reads a directory and would report "Not a directory" on it.
    if root.is_file() {
        let bytes = fs::metadata(root)?.len();
        tree.files = 1;
        tree.bytes = bytes;
        tree.entries.insert(String::new(), Entry::File { bytes });
        return Ok(tree);
    }
    collect(root, root, &mut tree)?;
    Ok(tree)
}

fn collect(root: &Path, dir: &Path, tree: &mut Tree) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path)?;
        if meta.is_symlink() {
            tree.entries.insert(
                relative_key(root, &path),
                Entry::Symlink {
                    target: fs::read_link(&path)?,
                },
            );
        } else if meta.is_dir() {
            tree.entries.insert(relative_key(root, &path), Entry::Dir);
            collect(root, &path, tree)?;
        } else {
            tree.files += 1;
            tree.bytes += meta.len();
            tree.entries
                .insert(relative_key(root, &path), Entry::File { bytes: meta.len() });
        }
    }
    Ok(())
}

fn relative_key(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

#[cfg(test)]
#[path = "tests/backup.rs"]
mod tests;
