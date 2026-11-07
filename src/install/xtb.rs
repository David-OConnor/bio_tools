//! xTB from its official binary releases: <https://github.com/grimme-lab/xtb/releases>.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use super::{
    InstallError, Installer,
    common::{ScratchDir, executable_name},
};

/// Pinned: Dynamics parses xTB's text output, so retest GFN-FF before changing this.
const VERSION: &str = "6.8.0";
const RELEASES: &str = "https://github.com/grimme-lab/xtb/releases/download";

/// Where the recipe places xTB, whether or not it is installed.
pub(super) fn executable(installer: &Installer) -> PathBuf {
    installer
        .tools_root()
        .join("xtb")
        .join("bin")
        .join(executable_name("xtb"))
}

pub(super) fn install(installer: &mut Installer) -> Result<(), InstallError> {
    let target = installer.tools_root().join("xtb");
    let executable = executable(installer);
    let installed = fs::read_to_string(target.join(".version")).unwrap_or_default();

    if installed.trim() == VERSION && executable.is_file() {
        installer.note(format!("xTB {VERSION} is already installed"));
    } else {
        install_release(installer, &target)?;
    }

    let mut verify = Command::new(executable);
    verify.arg("--version");
    installer.checked(&mut verify)?;
    installer.note(format!("xTB installed at {}", target.display()));
    Ok(())
}

fn install_release(installer: &Installer, target: &Path) -> Result<(), InstallError> {
    let asset = asset_name()?;
    let scratch = ScratchDir::new_in(installer.tools_root(), "xtb")?;
    let archive = scratch.path().join(&asset);
    installer.download(&format!("{RELEASES}/v{VERSION}/{asset}"), &archive)?;
    installer.extract_archive(&archive, scratch.path())?;

    let unpacked = scratch.path().join("stage");
    if !unpacked.join("bin").join(executable_name("xtb")).is_file() {
        return Err(InstallError::InvalidConfiguration(format!(
            "the xTB archive has an unexpected layout under {}",
            unpacked.display()
        )));
    }

    let old = scratch.path().join("previous-install");
    if target.exists() {
        fs::rename(target, &old).map_err(|error| {
            InstallError::io(format!("unable to stage {}", target.display()), error)
        })?;
    }
    if let Err(error) = fs::rename(&unpacked, target) {
        if old.exists() {
            let _ = fs::rename(&old, target);
        }
        return Err(InstallError::io(
            format!("unable to install xTB into {}", target.display()),
            error,
        ));
    }

    fs::write(target.join(".version"), format!("{VERSION}\n"))
        .map_err(|error| InstallError::io("unable to write the xTB version marker", error))
}

/// This platform's archive, as named on the releases page.
fn asset_name() -> Result<String, InstallError> {
    let platform = match (env::consts::OS, env::consts::ARCH) {
        ("linux", "x86_64") => "linux-x86_64-ifx-mkl.tar.xz",
        ("linux", "aarch64") => "linux-aarch64-openblas.tar.xz",
        ("macos", "x86_64") => "macos-x86_64-openblas.tar.xz",
        ("macos", "aarch64") => "macos-arm64-openblas.tar.xz",
        ("windows", "x86_64") => "windows-x86_64-openblas.zip",
        (os, arch) => {
            return Err(InstallError::InvalidConfiguration(format!(
                "xTB publishes no {VERSION} release for {os} on {arch}"
            )));
        }
    };

    Ok(format!("xtb-{VERSION}-{platform}"))
}
