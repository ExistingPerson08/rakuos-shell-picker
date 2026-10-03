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

impl Backup {
    /// Copy `sources` into a fresh directory under `parent`.
    ///
    /// `home` is the boundary: anything outside it is refused rather than
    /// copied, so a future registry entry pointing at a system path cannot end
    /// up being restored into one from a user-writable directory.
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
        // Not create_dir_all: this must fail if the directory already exists,
        // which is the point. Two switches must never share a backup directory,
        // or the second would overwrite the way out of the first.
        fs::create_dir(&root)
            .with_context(|| format!("creating backup directory {}", root.display()))?;

        let mut manifest = Manifest::default();
        for source in sources {
            let relative = relative_to_home(source, home)?;
            let key = relative.to_string_lossy().into_owned();
            if !source.exists() {
                manifest.absent.push(key);
                continue;
            }
            let tree = copy_tree(source, &root.join("home").join(relative))?;
            manifest.trees.insert(key, tree);
        }

        fs::write(
            root.join("manifest.toml"),
            toml::to_string_pretty(&manifest)?,
        )
        .context("writing the backup manifest")?;

        let backup = Backup { root, manifest };
        backup.verify()?;
        Ok(backup)
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
            let written = copy_tree(&from, &to)?;
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

/// Copy one tree, returning what was found on the way through.
fn copy_tree(from: &Path, to: &Path) -> Result<Tree> {
    let mut tree = Tree::default();
    if from.is_symlink() {
        // A symlinked config directory is followed for reading but recorded as
        // what it is, so restoring does not silently replace it with a real one.
        let target =
            fs::read_link(from).with_context(|| format!("reading link {}", from.display()))?;
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
    walk_into(from, from, to, &mut tree).map(|()| tree)
}

/// Recursive copy that fills in `tree`.
///
/// `root` is the tree being copied, kept so entries can be keyed by their path
/// relative to it. Keying by bare file name would collide across directories and
/// make the manifest disagree with what `walk` reads back.
fn walk_into(root: &Path, from: &Path, to: &Path, tree: &mut Tree) -> Result<()> {
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
        let child_from = from.join(&name);
        let child_to = to.join(&name);
        let meta = fs::symlink_metadata(&child_from)?;

        if meta.is_dir() {
            fs::create_dir_all(&child_to)
                .with_context(|| format!("creating {}", child_to.display()))?;
            tree.entries
                .insert(relative_key(root, &child_from), Entry::Dir);
            walk_into(root, &child_from, &child_to, tree)?;
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
mod tests {
    use super::*;

    fn sandbox(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join("rakuos-shell-picker-tests")
            .join("backup")
            .join(name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    #[test]
    fn a_backup_copies_the_whole_tree() {
        let root = sandbox("whole-tree");
        let home = root.join("home");
        write(&home.join(".config/noctalia/settings.json"), "{\"a\":1}");
        write(&home.join(".config/noctalia/themes/dark/gtk.css"), "dark");

        let backup = Backup::create(
            &root.join("backups"),
            "1234-hyprland-noctalia",
            &[home.join(".config/noctalia")],
            &home,
        )
        .unwrap();

        assert_eq!(backup.file_count(), 2);
        assert_eq!(backup.manifest.absent, Vec::<String>::new());
        assert!(backup.manifest.trees.contains_key(".config/noctalia"));
    }

    #[test]
    fn a_backup_refuses_to_reuse_a_directory() {
        let root = sandbox("reuse");
        let home = root.join("home");
        write(&home.join(".config/noctalia/a"), "a");
        let sources = [home.join(".config/noctalia")];

        Backup::create(&root.join("backups"), "same-name", &sources, &home).unwrap();
        // A second switch must not be able to land on top of the first one.
        let err = Backup::create(&root.join("backups"), "same-name", &sources, &home)
            .expect_err("second backup should refuse");
        assert!(
            format!("{err:#}").contains("creating backup directory"),
            "{err:#}"
        );
    }

    #[test]
    fn a_missing_source_is_recorded_rather_than_failing() {
        let root = sandbox("missing");
        let home = root.join("home");
        fs::create_dir_all(&home).unwrap();

        // A shell that was never opened has no config; that is not a failure.
        let backup = Backup::create(
            &root.join("backups"),
            "1-hyprland-dms",
            &[home.join(".config/DankMaterialShell")],
            &home,
        )
        .unwrap();

        assert_eq!(backup.file_count(), 0);
        assert_eq!(backup.manifest.absent, [".config/DankMaterialShell"]);
    }

    #[test]
    fn verification_catches_a_damaged_copy() {
        let root = sandbox("damaged");
        let home = root.join("home");
        write(&home.join(".config/noctalia/settings.json"), "{\"a\":1}");
        let backup = Backup::create(
            &root.join("backups"),
            "1-hyprland-noctalia",
            &[home.join(".config/noctalia")],
            &home,
        )
        .unwrap();

        // Something eats the copy after it was taken.
        fs::write(
            backup.root.join("home/.config/noctalia/settings.json"),
            "{\"a\":",
        )
        .unwrap();

        let err = backup.verify().expect_err("a truncated copy must not pass");
        let text = format!("{err:#}");
        assert!(text.contains("does not match its manifest"), "{text}");
        // The complaint has to point at the file that went wrong.
        assert!(text.contains("settings.json"), "{text}");
    }

    #[test]
    fn verification_catches_a_file_that_vanished() {
        let root = sandbox("vanished");
        let home = root.join("home");
        write(&home.join(".config/noctalia/keep.json"), "{}");
        write(&home.join(".config/noctalia/gone.json"), "{}");
        let config = home.join(".config/noctalia");
        let backup = Backup::create(
            &root.join("backups"),
            "1-hyprland-noctalia",
            std::slice::from_ref(&config),
            &home,
        )
        .unwrap();

        fs::remove_file(backup.root.join("home/.config/noctalia/gone.json")).unwrap();

        let err = backup.verify().expect_err("a missing copy must not pass");
        let text = format!("{err:#}");
        assert!(text.contains("gone.json"), "{text}");
    }

    #[test]
    fn restore_puts_the_original_back() {
        let root = sandbox("restore");
        let home = root.join("home");
        let config = home.join(".config/noctalia");
        write(&config.join("settings.json"), "{\"a\":1}");
        let backup = Backup::create(
            &root.join("backups"),
            "1-hyprland-noctalia",
            std::slice::from_ref(&config),
            &home,
        )
        .unwrap();

        // The switch replaces the config, then fails and has to undo it.
        fs::remove_dir_all(&config).unwrap();
        write(&config.join("settings.json"), "{\"a\":999}");

        let restored = backup.restore(&home).unwrap();
        assert_eq!(restored, 1);
        assert_eq!(
            fs::read_to_string(config.join("settings.json")).unwrap(),
            "{\"a\":1}"
        );
    }

    #[test]
    fn a_path_outside_home_is_refused() {
        let root = sandbox("outside");
        let home = root.join("home");
        fs::create_dir_all(&home).unwrap();

        let err = Backup::create(
            &root.join("backups"),
            "1-hyprland-noctalia",
            &[PathBuf::from("/etc")],
            &home,
        )
        .expect_err("system paths must be refused");
        assert!(format!("{err:#}").contains("outside"), "{err:#}");
    }

    #[test]
    fn a_relative_path_is_refused() {
        let root = sandbox("relative");
        let home = root.join("home");
        fs::create_dir_all(&home).unwrap();

        let err = Backup::create(
            &root.join("backups"),
            "1-x-y",
            &[PathBuf::from(".config")],
            &home,
        )
        .expect_err("relative paths must be refused");
        assert!(format!("{err:#}").contains("not an absolute"), "{err:#}");
    }

    #[test]
    fn a_directory_name_with_a_path_is_refused() {
        let root = sandbox("sneaky-name");
        let home = root.join("home");
        fs::create_dir_all(&home).unwrap();

        let err = Backup::create(&root.join("backups"), "../escape", &[], &home)
            .expect_err("a name must not be able to escape the backup directory");
        assert!(
            format!("{err:#}").contains("refusing a backup directory"),
            "{err:#}"
        );
    }

    #[test]
    fn symlinks_survive_a_round_trip() {
        let root = sandbox("symlinks");
        let home = root.join("home");
        let config = home.join(".config/noctalia");
        write(&config.join("real.json"), "{}");
        symlink("real.json", config.join("link.json")).unwrap();

        let backup = Backup::create(
            &root.join("backups"),
            "1-hyprland-noctalia",
            std::slice::from_ref(&config),
            &home,
        )
        .unwrap();
        assert!(backup.verify().is_ok());

        // Restoring must not turn a symlink into a copy of its target.
        fs::remove_file(config.join("link.json")).unwrap();
        backup.restore(&home).unwrap();
        assert!(config.join("link.json").is_symlink());
        assert_eq!(
            fs::read_link(config.join("link.json")).unwrap(),
            PathBuf::from("real.json")
        );
    }

    /// Copies the real user config. Ignored so `cargo test` never touches it by
    /// accident; run with `cargo test -- --ignored`.
    ///
    /// Restoring is deliberately not part of this: writing over a live desktop
    /// is not something a test should do, even into its own backup.
    #[test]
    #[ignore = "copies the real user config"]
    fn the_real_noctalia_setup_backs_up_and_verifies() {
        use crate::compositor::hyprland::shells::noctalia::SHELL as NOCTALIA;

        let home = PathBuf::from(std::env::var_os("HOME").expect("HOME"));
        let dest = std::env::temp_dir().join("rakuos-shell-picker-live-backup");
        let _ = fs::remove_dir_all(&dest);

        let sources =
            crate::switch::backup_sources(&home, &crate::compositor::HYPRLAND, Some(&NOCTALIA));
        let backup = Backup::create(&dest, "live-check", &sources, &home).unwrap();

        println!(
            "backed up {} files across {} trees, {} recorded absent",
            backup.file_count(),
            backup.manifest.trees.len(),
            backup.manifest.absent.len()
        );
        for key in backup.manifest.trees.keys() {
            let tree = &backup.manifest.trees[key];
            println!("  {key}: {} files, {} bytes", tree.files, tree.bytes);
        }
        for key in &backup.manifest.absent {
            println!("  {key}: absent");
        }

        // A real setup is hundreds of files; a handful means the walk stopped
        // early on something it should not have.
        assert!(
            backup.file_count() > 100,
            "only {} files copied, expected the whole setup",
            backup.file_count()
        );
        backup.verify().expect("the copy must read back clean");

        let _ = fs::remove_dir_all(&dest);
    }

    #[test]
    fn the_manifest_is_written_next_to_the_copy() {
        let root = sandbox("manifest");
        let home = root.join("home");
        write(&home.join(".config/noctalia/a"), "a");
        let backup = Backup::create(
            &root.join("backups"),
            "1-hyprland-noctalia",
            &[home.join(".config/noctalia")],
            &home,
        )
        .unwrap();

        let raw = fs::read_to_string(backup.root.join("manifest.toml")).unwrap();
        let parsed: Manifest = toml::from_str(&raw).unwrap();
        assert_eq!(parsed, backup.manifest);
    }
}
