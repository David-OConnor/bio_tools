//! One directory for the model weights and reference data tools download on first use.
//!
//! Left to their own defaults, tools scatter these across the home directory -- `~/.boltz`,
//! `~/.cache/opendde`, `~/checkpoint` for Protenix, `~/.cache/huggingface` and `~/.cache/torch`
//! for everything built on those libraries -- and Chai-1 keeps its weights inside its own
//! environment, so every reinstall fetched them again. Sharing between tools then happened only
//! by accident, when every process ran as the same user with the same home directory.
//!
//! Every tool process is instead pointed at one managed root through the variables each tool
//! already reads (see [`CACHES`]). The Hugging Face and Torch caches are content-addressed, so the
//! tools sharing them each fetch a given checkpoint once between them; the per-tool directories
//! are that tool's own, which is what lets an uninstall remove them.

use std::{
    fs,
    path::{Path, PathBuf},
};

use super::{Installer, UninstallReport, first_env_path, home_dir};
use crate::{status, tool_definitions::Tool};

/// Overrides where the managed caches live, for an operator who wants them on a particular disk.
const ROOT_VARIABLE: &str = "BIO_TOOLS_MODEL_CACHE";

/// A download directory a tool reads from an environment variable.
pub(super) struct Cache {
    pub variable: &'static str,
    /// Its subdirectory of the managed root.
    pub directory: &'static str,
    /// The tool whose downloads these are; `None` for a library cache several tools share.
    pub owner: Option<Tool>,
}

/// Every cache variable a managed tool process is given. The upstream sources are the authority
/// for each name: `huggingface_hub` and `torch.hub`, then `boltz.main.get_cache_path`,
/// `chai_lab.utils.paths`, `opendde.config.data`, `protenix.web_service.colab_request_parser`
/// and `catpred.data.cache_utils`.
pub(super) const CACHES: &[Cache] = &[
    Cache {
        variable: "HF_HOME",
        directory: "huggingface",
        owner: None,
    },
    Cache {
        variable: "TORCH_HOME",
        directory: "torch",
        owner: None,
    },
    Cache {
        variable: "BOLTZ_CACHE",
        directory: "boltz",
        owner: Some(Tool::Boltz2),
    },
    Cache {
        variable: "CHAI_DOWNLOADS_DIR",
        directory: "chai1",
        owner: Some(Tool::Chai1),
    },
    Cache {
        variable: "OPENDDE_ROOT_DIR",
        directory: "opendde",
        owner: Some(Tool::OpenDde),
    },
    Cache {
        variable: "PROTENIX_ROOT_DIR",
        directory: "protenix",
        owner: Some(Tool::Protenix),
    },
    Cache {
        variable: "CATPRED_CACHE_PATH",
        directory: "catpred",
        owner: Some(Tool::CatPred),
    },
];

impl Installer {
    /// The directory holding every managed download cache.
    ///
    /// Beside the tools it serves, except on a WSL mount of a Windows drive: those caches are
    /// tens of gigabytes read on every run, DrvFS is slow at that, and the Hugging Face cache links
    /// each snapshot to its blobs with symlinks DrvFS rejects, silently storing every file twice.
    pub fn model_cache_root(&self) -> PathBuf {
        first_env_path(&[ROOT_VARIABLE])
            .unwrap_or_else(|| self.native_data_dir("model_cache", self.tools_root()))
    }

    /// The value each cache variable is given: the operator's own where they set one, and the
    /// managed directory otherwise. Directories are created so that a tool which locks or writes
    /// into its cache before creating it -- Chai-1 does -- finds one there.
    pub fn model_cache_environment(&self) -> Vec<(&'static str, PathBuf)> {
        CACHES
            .iter()
            .map(|cache| {
                let path = self.cache_path(cache);
                let _ = fs::create_dir_all(&path);
                (cache.variable, path)
            })
            .collect()
    }

    /// The download directory `tool` owns, if it has one.
    pub(super) fn tool_cache_dir(&self, tool: Tool) -> Option<PathBuf> {
        owned(tool).next().map(|cache| self.cache_path(cache))
    }

    pub(super) fn cache_path(&self, cache: &Cache) -> PathBuf {
        if cache.owner == Some(Tool::OpenDde)
            && let Some(root) = &self.config.opendde_root
        {
            return root.clone();
        }
        first_env_path(&[cache.variable])
            .unwrap_or_else(|| self.model_cache_root().join(cache.directory))
    }

    /// Move a tool's existing downloads from where it kept them by default into the managed
    /// cache, so installing after this change does not fetch gigabytes that are already on disk.
    ///
    /// Only a directory the operator did not choose is moved, and only into an empty target. A
    /// move that cannot be a rename -- across filesystems -- is left for the tool to re-download,
    /// with the old copy named so it can be deleted.
    pub(super) fn adopt_legacy_caches(&self, tool: Tool) {
        self.note_shared_caches();
        for cache in owned(tool) {
            if first_env_path(&[cache.variable]).is_some()
                || (tool == Tool::OpenDde && self.config.opendde_root.is_some())
            {
                continue;
            }
            let target = self.cache_path(cache);
            for (from, to) in legacy_locations(self, tool, &target) {
                if !from.exists() || is_populated(&to) {
                    continue;
                }
                if let Some(parent) = to.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                // The target may be the empty directory `model_cache_environment` made.
                let _ = fs::remove_dir(&to);
                match fs::rename(&from, &to) {
                    Ok(()) => self.note(format!(
                        "Moved {} into the shared model cache at {}",
                        from.display(),
                        to.display()
                    )),
                    Err(error) => self.note(format!(
                        "Could not move {} to {} ({error}); it will be downloaded again there, \
                         and the old copy can be deleted",
                        from.display(),
                        to.display()
                    )),
                }
            }
        }
    }

    /// Point out a Hugging Face or Torch cache of the reader's own that the managed one is about
    /// to duplicate. Never moved for them: unlike a tool's own directory, these are where every
    /// other program on the machine keeps its models too.
    fn note_shared_caches(&self) {
        for cache in CACHES.iter().filter(|cache| cache.owner.is_none()) {
            let managed = self.cache_path(cache);
            let Some(legacy) = shared_default(cache) else {
                continue;
            };
            if managed != legacy && is_populated(&legacy) && !is_populated(&managed) {
                self.note(format!(
                    "{} already holds downloaded models. Tools here now use {}; moving the old                      directory there saves downloading them again",
                    legacy.display(),
                    managed.display()
                ));
            }
        }
    }
}

/// Where a library keeps its cache when nothing points it anywhere.
fn shared_default(cache: &Cache) -> Option<PathBuf> {
    let home = home_dir()?;
    match cache.variable {
        "HF_HOME" => Some(home.join(".cache").join("huggingface")),
        "TORCH_HOME" => Some(home.join(".cache").join("torch")),
        _ => None,
    }
}

/// Remove the caches `tool` owns, whatever an older install left in the home directory, and --
/// once no managed tool remains -- the library caches every tool shared.
pub(super) fn remove(installer: &Installer, tool: Tool, report: &mut UninstallReport) {
    for cache in owned(tool) {
        let path = installer.cache_path(cache);
        if path.starts_with(installer.model_cache_root()) {
            remove_quietly(installer, &path, report);
        } else {
            report.kept.push(format!(
                "{} is set to {}, outside the managed model cache; remove it by hand if you want \
                 the space back.",
                cache.variable,
                path.display()
            ));
        }
        for (legacy, _) in legacy_locations(installer, tool, &path) {
            let file = legacy.is_file();
            remove_quietly(installer, &legacy, report);
            // Protenix's files sit in `~/common` and `~/checkpoint`, ordinary names that may hold
            // something of the operator's too: those directories go only when left empty.
            if file && let Some(parent) = legacy.parent() {
                let _ = fs::remove_dir(parent);
            }
        }
    }
    if Tool::ALL
        .iter()
        .any(|other| *other != tool && status::is_recorded(installer, *other))
    {
        return;
    }
    for cache in CACHES.iter().filter(|cache| cache.owner.is_none()) {
        let path = installer.cache_path(cache);
        if path.starts_with(installer.model_cache_root()) {
            remove_quietly(installer, &path, report);
        }
    }
    let _ = fs::remove_dir(installer.model_cache_root());
}

fn owned(tool: Tool) -> impl Iterator<Item = &'static Cache> {
    CACHES.iter().filter(move |cache| cache.owner == Some(tool))
}

/// Where `tool` put its downloads before they had a managed home, paired with where each belongs
/// under `target`.
fn legacy_locations(installer: &Installer, tool: Tool, target: &Path) -> Vec<(PathBuf, PathBuf)> {
    let Some(home) = home_dir() else {
        return Vec::new();
    };
    let whole = |from: PathBuf| vec![(from, target.to_path_buf())];
    match tool {
        Tool::Boltz2 => whole(home.join(".boltz")),
        Tool::OpenDde => whole(home.join(".cache").join("opendde")),
        Tool::CatPred => whole(home.join(".cache.esm2_embeddings")),
        // Chai-1 downloads into `<site-packages>/downloads` when nothing says otherwise.
        Tool::Chai1 => site_packages(&installer.venv_dir(tool.slug()))
            .into_iter()
            .map(|packages| (packages.join("downloads"), target.to_path_buf()))
            .collect(),
        // Protenix's default root is the home directory itself, so its files go by exact name.
        Tool::Protenix => super::python_tools::protenix_cache_files(&home)
            .into_iter()
            .filter_map(|file| {
                let relative = file.strip_prefix(&home).ok()?.to_path_buf();
                Some((file, target.join(relative)))
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn site_packages(environment: &Path) -> Vec<PathBuf> {
    let windows = environment.join("Lib").join("site-packages");
    let mut found: Vec<PathBuf> = fs::read_dir(environment.join("lib"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path().join("site-packages"))
        .collect();
    found.push(windows);
    found.retain(|path| path.is_dir());
    found
}

fn is_populated(path: &Path) -> bool {
    match fs::read_dir(path) {
        Ok(mut entries) => entries.next().is_some(),
        Err(_) => path.exists(),
    }
}

fn remove_quietly(installer: &Installer, path: &Path, report: &mut UninstallReport) {
    let Ok(metadata) = path.symlink_metadata() else {
        return;
    };
    let outcome = if metadata.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
    match outcome {
        Ok(()) => {
            installer.step(format!("Removed {}", path.display()));
            report.removed.push(path.to_path_buf());
        }
        Err(error) => report.kept.push(format!(
            "{} could not be removed: {error}. It holds downloaded model data; remove it by hand \
             to reclaim the space.",
            path.display()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::{InstallConfig, InstallLayout};

    fn installer(root: &Path) -> Installer {
        let mut config = InstallConfig::new(root);
        config.layout = InstallLayout::process_executables(root);
        Installer::from_config(config)
    }

    fn scratch(name: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("bio-tools-{name}-{unique}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn every_cache_has_its_own_variable_and_directory() {
        let mut variables: Vec<&str> = CACHES.iter().map(|cache| cache.variable).collect();
        let mut directories: Vec<&str> = CACHES.iter().map(|cache| cache.directory).collect();
        let total = CACHES.len();
        variables.sort_unstable();
        variables.dedup();
        directories.sort_unstable();
        directories.dedup();
        assert_eq!(variables.len(), total);
        assert_eq!(directories.len(), total);
        // A tool owns at most one cache, since `tool_cache_dir` answers with one path.
        let mut owners: Vec<Tool> = CACHES.iter().filter_map(|cache| cache.owner).collect();
        let owned = owners.len();
        owners.sort_unstable_by_key(|tool| tool.slug());
        owners.dedup();
        assert_eq!(owners.len(), owned);
    }

    #[test]
    fn caches_are_one_directory_each_below_one_root() {
        let root = scratch("cache-root");
        let installer = installer(&root);
        let cache_root = installer.model_cache_root();
        let environment = installer.model_cache_environment();

        assert_eq!(environment.len(), CACHES.len());
        for (variable, path) in &environment {
            // Created rather than merely named: Chai-1 locks its download directory on first use.
            assert!(path.is_dir(), "{variable} was not created at {}", path.display());
            if std::env::var_os(variable).is_none() {
                assert!(path.starts_with(&cache_root));
                assert_eq!(path.parent(), Some(cache_root.as_path()));
            }
        }
        fs::remove_dir_all(&root).ok();
        fs::remove_dir_all(&cache_root).ok();
    }

    #[test]
    fn an_existing_download_directory_is_moved_rather_than_refetched() {
        let root = scratch("cache-adopt");
        let installer = installer(&root);
        let Some(target) = installer.tool_cache_dir(Tool::Chai1) else {
            panic!("Chai-1 owns a download cache");
        };
        // Chai-1's own default: inside the environment the install is about to rebuild.
        let legacy = installer
            .venv_dir(Tool::Chai1.slug())
            .join("lib")
            .join("python3.11")
            .join("site-packages")
            .join("downloads");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("weights.pt"), b"weights").unwrap();
        fs::create_dir_all(&target).unwrap();

        installer.adopt_legacy_caches(Tool::Chai1);

        assert!(!legacy.exists(), "the old copy was left behind");
        assert_eq!(
            fs::read_to_string(target.join("weights.pt")).unwrap(),
            "weights"
        );

        // A second install has nothing to move, and leaves what is already there alone.
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("weights.pt"), b"stale").unwrap();
        installer.adopt_legacy_caches(Tool::Chai1);
        assert_eq!(
            fs::read_to_string(target.join("weights.pt")).unwrap(),
            "weights"
        );

        fs::remove_dir_all(&root).ok();
        fs::remove_dir_all(installer.model_cache_root()).ok();
    }
}
