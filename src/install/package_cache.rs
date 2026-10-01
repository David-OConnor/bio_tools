//! The package caches behind the managed environments, kept from holding a second copy of
//! anything.
//!
//! uv installs by hard-linking each file out of its cache -- its default on Linux and Windows --
//! so every environment holding the same wheel shares one copy of it on disk. Most tool
//! environments hold the same multi-gigabyte PyTorch and CUDA libraries, so that sharing is most
//! of what they cost. It only works when the cache and the environments are on one filesystem,
//! though: otherwise uv falls back to copying, and every environment carries its own copy of each
//! library on top of the one in the cache. That is the case for an application kept on a Windows
//! drive under WSL, whose environments are on `/mnt/c` while uv's cache is in the Linux home
//! directory, and for environments on a second drive on Windows.
//!
//! There, the environments get a uv cache of their own beside them ([`OWNED_UV_CACHE`]). Nothing
//! else uses that cache, so after each install and uninstall, the wheels no environment links to
//! any more are pruned from it: the versions an upgrade replaced, and the libraries only an
//! uninstalled tool used. uv's own cache is never pruned; it also serves projects that are not
//! ours.
//!
//! Either way, a file in an environment may be a hard link shared with the cache and with every
//! other environment that holds the same wheel. Anything that changes an installed file must
//! replace it -- write a new file, then rename it over the old one -- rather than edit it in
//! place, or the edit lands in all of them.
//!
//! micromamba and Conda keep each package's downloaded archive beside the unpacked copy their
//! environments link to. The archive is only needed again if that unpacked copy goes, so it is
//! deleted once an install is done.

use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

use super::{InstallError, Installer, common::ScratchDir, first_env};

/// The uv cache kept beside the environments when uv's own cannot be linked to them.
const OWNED_UV_CACHE: &str = "uv-cache";

impl Installer {
    /// A uv invocation that installs by linking out of a cache on the environments' filesystem.
    pub(crate) fn uv_command(&mut self) -> Result<Command, InstallError> {
        let uv = self.ensure_uv()?;
        let mut command = Command::new(&uv);
        if let Some(cache) = self.owned_uv_cache(&uv) {
            command.env("UV_CACHE_DIR", cache);
        }
        Ok(command)
    }

    /// The cache uv should use in place of its own, decided once per run.
    fn owned_uv_cache(&mut self, uv: &Path) -> Option<PathBuf> {
        if let Some(decided) = &self.owned_uv_cache {
            return decided.clone();
        }
        let decided = self.choose_uv_cache(uv);
        if let Some(cache) = &decided {
            self.note(format!(
                "Using {} as uv's cache: uv's own is on another filesystem, which it can only copy \
                 packages out of",
                cache.display()
            ));
        }
        self.owned_uv_cache = Some(decided.clone());
        decided
    }

    fn choose_uv_cache(&self, uv: &Path) -> Option<PathBuf> {
        // An operator's own choice stands, even one uv can only copy out of.
        if first_env(&["UV_CACHE_DIR", "UV_NO_CACHE"]).is_some() {
            return None;
        }
        let environments = &self.config.layout.environments_root;
        let owned = environments.join(OWNED_UV_CACHE);
        // Once environments link into it, it stays the cache they install from.
        if owned.is_dir() {
            return Some(owned);
        }
        let mut probe = Command::new(uv);
        probe.args(["cache", "dir"]);
        let output = self.capture(&mut probe).ok()?;
        let default = PathBuf::from(String::from_utf8(output.stdout).ok()?.trim());
        if default.as_os_str().is_empty() || can_hard_link(&default, environments) {
            return None;
        }
        // On a filesystem with no hard links at all (FAT, some network shares), a second cache
        // would be copied out of just the same.
        can_hard_link(environments, environments).then_some(owned)
    }

    /// Remove the wheels no environment links to any more from the owned uv cache.
    pub(crate) fn prune_uv_cache(&mut self) {
        let cache = self.config.layout.environments_root.join(OWNED_UV_CACHE);
        if !cache.is_dir() || release_unused_wheels(&cache) == 0 {
            return;
        }
        // uv deletes the archives no pointer reaches any more, under its own cache lock.
        let Ok(uv) = self.ensure_uv() else {
            return;
        };
        let mut prune = Command::new(uv);
        prune.args(["cache", "prune"]).env("UV_CACHE_DIR", &cache);
        if !self.succeeds(&mut prune) {
            self.note(format!(
                "`uv cache prune` did not succeed; {} may hold packages no tool uses",
                cache.display()
            ));
        }
    }

    /// Delete the package archives kept by the micromamba and Conda this run used.
    pub(crate) fn clean_package_archives(&mut self) {
        if self.micromamba.is_some()
            && let Ok(mut command) = self.micromamba_command()
        {
            command.args(["clean", "--tarballs", "--yes"]);
            let _ = self.succeeds(&mut command);
        }
        if let Some(conda) = self.conda.clone() {
            let mut command = Command::new(conda);
            command.args(["clean", "--tarballs", "--yes"]);
            let _ = self.succeeds(&mut command);
        }
    }
}

/// Whether a file under `from` can be hard-linked into `to`: a shared filesystem that supports
/// hard links.
fn can_hard_link(from: &Path, to: &Path) -> bool {
    let (Ok(source), Ok(target)) = (
        ScratchDir::new_in(from, "link-probe"),
        ScratchDir::new_in(to, "link-probe"),
    ) else {
        return false;
    };
    let file = source.path().join("probe");
    fs::write(&file, b"").is_ok() && fs::hard_link(&file, target.path().join("probe")).is_ok()
}

/// Remove uv's pointers to the wheels no environment links to, returning how many went.
///
/// uv points at each unpacked wheel (`archive-v*/<id>`) from its `wheels-v*` and `sdists-v*`
/// buckets -- with a symlink on Unix, and on Windows with a small file holding the archive's path
/// relative to the cache -- and `uv cache prune` then deletes archives no pointer reaches.
/// Pointers go first, never archives: uv replaces a pointer whose archive has vanished by renaming
/// a new one over it, which DrvFS refuses for a symlink, so the next install of that wheel would
/// fail. A pointer that already dangles goes for the same reason.
fn release_unused_wheels(cache: &Path) -> usize {
    let archives: Vec<PathBuf> = buckets(cache, "archive-v")
        .filter_map(|bucket| fs::canonicalize(bucket).ok())
        .collect();
    let mut pointers = Vec::new();
    for bucket in buckets(cache, "wheels-v").chain(buckets(cache, "sdists-v")) {
        collect_pointers(&bucket, &mut pointers);
    }
    pointers
        .into_iter()
        .filter(|pointer| match target(cache, pointer) {
            None => true,
            Some(archive) => {
                archive
                    .parent()
                    .is_some_and(|parent| archives.iter().any(|bucket| bucket == parent))
                    && !wheel_in_use(&archive)
            }
        })
        .filter(|pointer| remove_link(pointer).is_ok())
        .count()
}

fn buckets<'a>(cache: &Path, prefix: &'a str) -> impl Iterator<Item = PathBuf> + 'a {
    fs::read_dir(cache)
        .into_iter()
        .flatten()
        .flatten()
        .filter(move |entry| entry.file_name().to_string_lossy().starts_with(prefix))
        .map(|entry| entry.path())
}

fn collect_pointers(directory: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).into_iter().flatten().flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if kind.is_symlink() || (kind.is_file() && link_file_target(&path).is_some()) {
            found.push(path);
        } else if kind.is_dir() && entry.file_name() != "src" {
            // `src` is an unpacked source distribution, whose own symlinks are not uv's.
            collect_pointers(&path, found);
        }
    }
}

/// The canonical directory a pointer leads to, or `None` where it leads nowhere.
fn target(cache: &Path, pointer: &Path) -> Option<PathBuf> {
    let destination = match link_file_target(pointer) {
        Some(relative) => cache.join(relative),
        None => pointer.to_path_buf(),
    };
    fs::canonicalize(destination).ok()
}

/// What a Windows pointer file names: `archive-v0/<id>`, and nothing else uv keeps beside it.
fn link_file_target(path: &Path) -> Option<String> {
    const SIDECARS: [&str; 5] = [".http", ".msgpack", ".rev", ".lock", ".whl"];
    let name = path.file_name()?.to_string_lossy();
    if SIDECARS.iter().any(|suffix| name.ends_with(suffix)) {
        return None;
    }
    let metadata = path.symlink_metadata().ok()?;
    if !metadata.is_file() || metadata.len() > 128 {
        return None;
    }
    let content = fs::read_to_string(path).ok()?;
    let relative = content.trim();
    let (bucket, id) = relative.split_once('/')?;
    (bucket.starts_with("archive-v") && !id.is_empty() && !id.contains(['/', '\\', '.']))
        .then(|| relative.to_owned())
}

/// Whether any environment still links to the wheel unpacked at `archive`. uv writes each
/// environment a `RECORD` of its own, but links `METADATA` like every other file.
fn wheel_in_use(archive: &Path) -> bool {
    let metadata = fs::read_dir(archive)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|extension| extension == "dist-info"))
        .map(|dist_info| dist_info.join("METADATA"));
    // Keep what this cannot tell about: a wrong guess the other way costs a download.
    metadata
        .and_then(|path| link_count(&path))
        .is_none_or(|links| links > 1)
}

/// A pointer file, a symlink to a file or a directory, or a Windows junction.
fn remove_link(path: &Path) -> io::Result<()> {
    fs::remove_file(path).or_else(|_| fs::remove_dir(path))
}

#[cfg(unix)]
fn link_count(path: &Path) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    fs::metadata(path).ok().map(|metadata| metadata.nlink())
}

/// `std` reports link counts on Windows only behind an unstable feature, so this asks the file
/// system directly.
#[cfg(windows)]
fn link_count(path: &Path) -> Option<u64> {
    use std::{ffi::c_void, mem::MaybeUninit, os::windows::io::AsRawHandle};

    /// `BY_HANDLE_FILE_INFORMATION`; each `FILETIME` is two `u32`s.
    #[repr(C)]
    struct FileInformation {
        attributes: u32,
        creation_time: [u32; 2],
        last_access_time: [u32; 2],
        last_write_time: [u32; 2],
        volume_serial_number: u32,
        size_high: u32,
        size_low: u32,
        number_of_links: u32,
        index_high: u32,
        index_low: u32,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetFileInformationByHandle(file: *mut c_void, information: *mut FileInformation)
        -> i32;
    }

    let file = fs::File::open(path).ok()?;
    let mut information = MaybeUninit::<FileInformation>::uninit();
    // SAFETY: the handle belongs to `file`, which outlives the call, and the function fills the
    // whole structure whenever it reports success.
    let succeeded =
        unsafe { GetFileInformationByHandle(file.as_raw_handle(), information.as_mut_ptr()) } != 0;
    // SAFETY: initialised by the successful call above.
    succeeded.then(|| u64::from(unsafe { information.assume_init() }.number_of_links))
}

#[cfg(not(any(unix, windows)))]
fn link_count(_path: &Path) -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("bio-tools-{name}-{unique}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    /// A directory symlink, or `None` where this account may not create one (Windows without
    /// Developer Mode).
    fn symlink_dir(target: &Path, link: &Path) -> Option<()> {
        #[cfg(unix)]
        let made = std::os::unix::fs::symlink(target, link);
        #[cfg(windows)]
        let made = std::os::windows::fs::symlink_dir(target, link);
        made.ok()
    }

    /// An unpacked wheel in the cache's archive bucket.
    fn archive(cache: &Path, id: &str) -> PathBuf {
        let archive = cache.join("archive-v0").join(id);
        let dist_info = archive.join("example-1.0.dist-info");
        fs::create_dir_all(&dist_info).unwrap();
        fs::write(dist_info.join("METADATA"), id).unwrap();
        archive
    }

    #[test]
    fn hard_links_are_counted() {
        let root = scratch("link-count");
        let file = root.join("a");
        fs::write(&file, b"").unwrap();
        assert_eq!(link_count(&file), Some(1));
        fs::hard_link(&file, root.join("b")).unwrap();
        assert_eq!(link_count(&file), Some(2));
        assert!(can_hard_link(&root, &root));
        fs::remove_dir_all(&root).ok();
    }

    /// A cache holding one wheel an environment links to and one nothing does, and the pointers
    /// to each that `point` makes, plus one to an archive that is gone.
    fn released_pointers(name: &str, point: fn(&Path, &Path, &Path) -> Option<()>) {
        let cache = scratch(name);
        let used = archive(&cache, "used");
        let unused = archive(&cache, "unused");
        let environment = cache.join("environment");
        fs::create_dir_all(&environment).unwrap();
        fs::hard_link(
            used.join("example-1.0.dist-info").join("METADATA"),
            environment.join("METADATA"),
        )
        .unwrap();

        let pointers = cache.join("wheels-v6").join("pypi").join("example");
        fs::create_dir_all(&pointers).unwrap();
        // The siblings uv keeps beside each pointer, which are not pointers themselves.
        fs::write(pointers.join("1.0-py3-none-any.http"), b"archive-v0/used").unwrap();
        fs::write(pointers.join("1.0-py3-none-any.msgpack"), [0x93, 0x01]).unwrap();
        let keep = pointers.join("1.0-py3-none-any");
        let release = pointers.join("1.0-cp312-cp312-linux_x86_64");
        let dangling = pointers.join("1.0-cp311-cp311-linux_x86_64");
        if point(&cache, &used, &keep).is_none() {
            fs::remove_dir_all(&cache).ok();
            return;
        }
        point(&cache, &unused, &release).unwrap();
        point(&cache, &cache.join("archive-v0").join("gone"), &dangling).unwrap();

        assert_eq!(release_unused_wheels(&cache), 2);
        assert!(keep.symlink_metadata().is_ok());
        assert!(release.symlink_metadata().is_err());
        assert!(dangling.symlink_metadata().is_err());
        assert!(pointers.join("1.0-py3-none-any.http").is_file());
        assert!(pointers.join("1.0-py3-none-any.msgpack").is_file());
        // The archives themselves are left for `uv cache prune`.
        assert!(unused.is_dir());
        fs::remove_dir_all(&cache).ok();
    }

    #[test]
    fn only_symlinks_to_unlinked_wheels_are_released() {
        released_pointers("uv-prune-symlinks", |_, archive, pointer| {
            symlink_dir(archive, pointer)
        });
    }

    #[test]
    fn only_pointer_files_to_unlinked_wheels_are_released() {
        released_pointers("uv-prune-files", |cache, archive, pointer| {
            let relative = archive.strip_prefix(cache).unwrap().to_string_lossy();
            fs::write(pointer, relative.replace('\\', "/")).ok()
        });
    }
}
