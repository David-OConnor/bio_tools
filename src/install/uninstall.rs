//! Removal of a tool one of this crate's recipes installed.
//!
//! Only what a recipe created is removed: the tool's isolated environment, the checkouts and
//! binary distributions it unpacked under the tools root, and its installation marker. Shared
//! infrastructure -- the micromamba root, the bootstrapped Conda, the uv cache -- belongs to every
//! other tool as well and is left alone, as are the multi-gigabyte assets that are expensive to
//! fetch and are not owned by the tool that happened to download them.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use super::{InstallError, Installer};
use crate::{status, tool_definitions::Tool};

/// What [`Installer::uninstall`] removed, and what it deliberately left behind.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UninstallReport {
    pub removed: Vec<PathBuf>,
    /// Human-readable notes about anything kept on purpose, so an operator who wants the disk
    /// space back knows what is still there and why nothing removed it for them.
    pub kept: Vec<String>,
}

impl UninstallReport {
    /// Whether anything was actually there to remove.
    pub fn removed_anything(&self) -> bool {
        !self.removed.is_empty()
    }
}

pub(super) fn uninstall(
    installer: &mut Installer,
    tool: Tool,
) -> Result<UninstallReport, InstallError> {
    let mut report = UninstallReport::default();

    // Before the directories: a named Conda environment is removed by Conda, and the recipes that
    // create one drive an upstream installer that puts it in Conda's own envs directory rather
    // than in the layout below.
    if let Some(name) = tool.conda_environment() {
        remove_conda_environment(installer, name, &mut report);
    }

    for path in removable_paths(installer, tool, &mut report) {
        remove(installer, &path, &mut report)?;
    }

    for note in tool.retained_assets() {
        report.kept.push((*note).to_owned());
    }

    status::forget_install(installer, tool)?;
    Ok(report)
}

/// Everything on disk this tool's recipe owns, whether or not it currently exists.
fn removable_paths(
    installer: &Installer,
    tool: Tool,
    report: &mut UninstallReport,
) -> Vec<PathBuf> {
    let mut paths = vec![installer.venv_dir(tool.slug())];
    if let Some(name) = tool.conda_environment() {
        let compatibility = installer
            .config
            .layout
            .environments_root
            .join("conda")
            .join("envs")
            .join(name);
        if installer.environment_path(tool) != compatibility {
            paths.push(compatibility);
        }
    }
    // GROMACS is the one recipe whose install prefix an operator may point anywhere. Inside the
    // managed tree it is ours to remove; outside it, they chose that location and may well have
    // put other things there, so it stays.
    if tool == Tool::Gromacs
        && let Some(prefix) = installer.config.gromacs_prefix.clone()
    {
        if prefix.starts_with(installer.tools_root()) {
            paths.push(prefix);
        } else {
            report.kept.push(format!(
                "GROMACS_INSTALL_PREFIX points at {}, outside the managed tree; remove it by \
                 hand if you want the space back.",
                prefix.display()
            ));
        }
    }
    paths.extend(
        tool.asset_directories()
            .iter()
            .map(|relative| installer.tools_root().join(relative)),
    );
    paths
}

fn remove(
    installer: &Installer,
    path: &Path,
    report: &mut UninstallReport,
) -> Result<(), InstallError> {
    // symlink_metadata, not exists(): a dangling symlink left by a half-finished install is still
    // something to remove, and exists() follows the link and answers no.
    let Ok(metadata) = path.symlink_metadata() else {
        return Ok(());
    };
    guard(installer, path)?;
    let outcome = if metadata.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
    outcome
        .map_err(|error| InstallError::io(format!("unable to remove {}", path.display()), error))?;
    installer.step(format!("Removed {}", path.display()));
    report.removed.push(path.to_path_buf());
    Ok(())
}

/// Refuse to delete anything that is not one of this installation's own subdirectories.
///
/// Nothing above should ever produce such a path, which is the point: a recursive delete driven
/// by a table of relative names is worth one explicit check, so that a layout misconfigured to
/// point at a home directory fails here instead of taking the tree with it.
fn guard(installer: &Installer, path: &Path) -> Result<(), InstallError> {
    let roots = [
        installer.tools_root().to_path_buf(),
        installer.config.layout.environments_root.clone(),
    ];
    if roots
        .iter()
        .any(|root| path != root && path.starts_with(root))
    {
        return Ok(());
    }
    Err(InstallError::InvalidConfiguration(format!(
        "{} is outside the managed installation tree; remove it manually",
        path.display()
    )))
}

fn remove_conda_environment(installer: &mut Installer, name: &str, report: &mut UninstallReport) {
    let Some(conda) = existing_conda(installer) else {
        report.kept.push(format!(
            "The Conda environment {name} was left in place: no Conda installation was found to \
             remove it with."
        ));
        return;
    };
    let mut command = Command::new(conda);
    command.args(["env", "remove", "--name", name, "-y"]);
    if installer.succeeds(&mut command) {
        installer.step(format!("Removed the Conda environment {name}"));
    } else {
        report.kept.push(format!(
            "`conda env remove --name {name}` did not succeed; the environment may not have \
             existed."
        ));
    }
}

/// A Conda that is already installed, never a freshly bootstrapped one: uninstalling must not
/// download the several hundred megabytes that [`Installer::ensure_conda`] would.
fn existing_conda(installer: &Installer) -> Option<PathBuf> {
    let conda = installer.conda_executable_path();
    conda.is_file().then_some(conda)
}
