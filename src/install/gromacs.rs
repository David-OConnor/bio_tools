//! GROMACS from prebuilt CUDA binaries; it is never compiled here.
//!
//! The project publishes source only. Linux uses conda-forge. conda-forge skips Windows, which
//! uses <https://github.com/Arifmaulanaazis/Gromacs-Prebuild-Windows>.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use super::{
    InstallError, Installer,
    common::{CONDA_FORGE, ScratchDir, executable_name},
};

/// The newest release with a Windows CUDA build. This is its AVX2 archive: the AVX-512 one crashes
/// on CPUs without AVX-512, and GROMACS's install guide notes AVX2 can be faster on GPU runs.
const WINDOWS_VERSION: &str = "2026.2";
const WINDOWS_RELEASES: &str =
    "https://github.com/Arifmaulanaazis/Gromacs-Prebuild-Windows/releases/download";
/// Pinned because a community release asset can be replaced after publication.
const WINDOWS_SHA256: &str = "c2de89055c7c0491914d4e116821d500e9be9ed52960e8171a7a423a9d009543";

/// Absent from source builds made by earlier versions of this recipe, so those get replaced.
const VERSION_MARKER: &str = ".version";

fn prefix(installer: &Installer) -> PathBuf {
    installer
        .config
        .gromacs_prefix
        .clone()
        .unwrap_or_else(|| installer.tools_root().join("gromacs"))
}

/// Where the recipe places `gmx`, whether or not it is installed.
pub(super) fn executable(installer: &Installer) -> PathBuf {
    prefix(installer).join("bin").join(executable_name("gmx"))
}

pub(super) fn install(installer: &mut Installer) -> Result<(), InstallError> {
    let target = prefix(installer);
    let executable = executable(installer);
    let version = if cfg!(target_os = "windows") {
        WINDOWS_VERSION.to_owned()
    } else {
        installer.config.gromacs_version.clone()
    };
    let installed = fs::read_to_string(target.join(VERSION_MARKER)).unwrap_or_default();

    if installed.trim() == version && executable.is_file() {
        installer.note(format!("GROMACS {version} is already installed"));
    } else {
        check_hardware(installer)?;

        if cfg!(target_os = "windows") {
            install_windows_release(installer, &target)?;
        } else {
            install_conda_package(installer, &target, &version)?;
        }

        fs::write(target.join(VERSION_MARKER), format!("{version}\n")).map_err(|error| {
            InstallError::io("unable to write the GROMACS version marker", error)
        })?;
    }

    verify_full_performance(installer, &executable)?;
    installer.note(format!(
        "GROMACS {version} installed at {}",
        target.display()
    ));
    Ok(())
}

/// Every prebuilt option here is x86-64 CUDA. `CONDA_OVERRIDE_CUDA` is honoured because it is how
/// micromamba is told to install CUDA packages on a host without a GPU, such as an image build.
fn check_hardware(installer: &Installer) -> Result<(), InstallError> {
    if env::consts::ARCH != "x86_64" {
        return Err(InstallError::InvalidConfiguration(format!(
            "no prebuilt CUDA GROMACS exists for {}",
            env::consts::ARCH
        )));
    }

    if installer.nvidia_driver_version().is_none() && env::var_os("CONDA_OVERRIDE_CUDA").is_none() {
        return Err(InstallError::InvalidConfiguration(
            "GROMACS is only installed as a CUDA build, and no NVIDIA GPU was found".to_owned(),
        ));
    }

    Ok(())
}

fn install_conda_package(
    installer: &mut Installer,
    target: &Path,
    version: &str,
) -> Result<(), InstallError> {
    clear_previous(installer, target)?;

    let mut create = installer.micromamba_command()?;
    create
        .args(["create", "--yes", "--prefix"])
        .arg(target)
        .args(["-c", CONDA_FORGE])
        // `nompi` is the thread-MPI build, whose binary is the `gmx` the adapters call. Its `bin/gmx`
        // is a wrapper that picks the best SIMD build for this CPU, so needs no activation.
        .arg(format!("gromacs={version}=nompi_cuda_*"));
    installer.checked(&mut create)
}

fn install_windows_release(installer: &Installer, target: &Path) -> Result<(), InstallError> {
    let asset = format!("Gromacs.{WINDOWS_VERSION}.Prebuild.Windows.X64.CUDA13.0.zip");
    let parent = target.parent().unwrap_or(installer.tools_root());
    let scratch = ScratchDir::new_in(parent, "gromacs")?;
    let archive = scratch.path().join(&asset);

    installer.download(
        &format!("{WINDOWS_RELEASES}/{WINDOWS_VERSION}/{asset}"),
        &archive,
    )?;
    check_sha256(installer, &archive)?;

    let unpacked = scratch.path().join("unpacked");
    installer.extract_archive(&archive, &unpacked)?;
    let root = release_root(&unpacked)?;

    clear_previous(installer, target)?;
    fs::rename(&root, target).map_err(|error| {
        InstallError::io(
            format!("unable to install GROMACS into {}", target.display()),
            error,
        )
    })
}

/// `certutil` ships with Windows, which is the only platform this runs on.
fn check_sha256(installer: &Installer, file: &Path) -> Result<(), InstallError> {
    let mut hash = Command::new("certutil");
    hash.arg("-hashfile").arg(file).arg("SHA256");
    let output = installer.capture(&mut hash)?;

    // Older Windows versions space the digits out.
    let matches = String::from_utf8_lossy(&output.stdout)
        .lines()
        .any(|line| line.replace(' ', "").eq_ignore_ascii_case(WINDOWS_SHA256));

    if matches {
        Ok(())
    } else {
        Err(InstallError::InvalidConfiguration(format!(
            "{} does not match its pinned SHA-256",
            file.display()
        )))
    }
}

/// The install tree in an unpacked release: the archive's root, or a single folder within it.
fn release_root(unpacked: &Path) -> Result<PathBuf, InstallError> {
    let children = fs::read_dir(unpacked)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path());

    std::iter::once(unpacked.to_path_buf())
        .chain(children)
        .find(|root| {
            root.join("bin").join(executable_name("gmx")).is_file()
                && root.join("share").join("gromacs").join("top").is_dir()
        })
        .ok_or_else(|| {
            InstallError::InvalidConfiguration(format!(
                "the GROMACS archive has an unexpected layout under {}",
                unpacked.display()
            ))
        })
}

/// Removes an earlier install. A `GROMACS_INSTALL_PREFIX` outside the managed tree may hold the
/// operator's other files, so it is only cleared if this recipe populated it.
fn clear_previous(installer: &Installer, target: &Path) -> Result<(), InstallError> {
    let Ok(mut entries) = fs::read_dir(target) else {
        return Ok(());
    };

    let ours = target.starts_with(installer.tools_root()) || target.join(VERSION_MARKER).is_file();
    if entries.next().is_some() && !ours {
        return Err(InstallError::InvalidConfiguration(format!(
            "GROMACS_INSTALL_PREFIX points at {}, which already holds other files; empty it or \
             choose another location",
            target.display()
        )));
    }

    fs::remove_dir_all(target)
        .map_err(|error| InstallError::io(format!("unable to clear {}", target.display()), error))
}

/// Runs the installed `gmx` and rejects a CPU-only or FFTPACK build, whatever it was labelled.
fn verify_full_performance(installer: &Installer, executable: &Path) -> Result<(), InstallError> {
    let output = installer.capture(Command::new(executable).arg("--version"))?;
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let has_cuda = report
        .lines()
        .any(|line| line.contains("GPU support") && line.contains("CUDA"));
    let has_slow_fft = report
        .lines()
        .any(|line| line.contains("FFT library") && line.to_lowercase().contains("fftpack"));

    if has_cuda && !has_slow_fft {
        Ok(())
    } else {
        Err(InstallError::InvalidConfiguration(format!(
            "{} is not a full-performance build (CUDA and a fast FFT library are required):\n{report}",
            executable.display()
        )))
    }
}
