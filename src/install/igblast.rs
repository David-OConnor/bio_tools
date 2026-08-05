use std::{fs, path::Path, process::Command};

use super::{
    InstallError, Installer,
    common::{ScratchDir, directory_contains_extension},
};

const RELEASE_ROOT: &str = "https://ftp.ncbi.nih.gov/blast/executables/igblast/release";
const DATABASE_ARCHIVES: [&str; 5] = [
    "database/airr/airr_c_human.tar",
    "database/airr/airr_c_mouse.tar",
    "database/mouse_gl_VDJ.tar",
    "database/rhesus_monkey_VJ.tar",
    "database/ncbi_human_c_genes.tar",
];

pub(super) fn install(installer: &mut Installer) -> Result<(), InstallError> {
    if !cfg!(target_arch = "x86_64") {
        return Err(InstallError::InvalidConfiguration(
            "NCBI publishes IgBLAST distributions for x86_64 only".to_owned(),
        ));
    }
    fs::create_dir_all(installer.tools_root()).map_err(|error| {
        InstallError::io(
            format!("unable to create {}", installer.tools_root().display()),
            error,
        )
    })?;
    let target = installer.tools_root().join("igblast");
    let marker = target.join(".version");
    let installed_version = fs::read_to_string(&marker)
        .ok()
        .map(|value| value.trim().to_owned());
    if installed_version.as_deref() != Some(&installer.config.igblast_version) {
        install_distribution(installer, &target)?;
    } else {
        installer.note(format!(
            "IgBLAST {} is already installed",
            installer.config.igblast_version
        ));
    }
    install_databases(installer, &target)?;

    let executable = target.join("bin").join(if cfg!(target_os = "windows") {
        "igblastn.exe"
    } else {
        "igblastn"
    });
    let mut verify = Command::new(executable);
    verify.arg("-version").env("IGDATA", &target);
    installer.checked(&mut verify)?;
    installer.note(format!("IgBLAST installed at {}", target.display()));
    Ok(())
}

fn install_distribution(installer: &Installer, target: &Path) -> Result<(), InstallError> {
    let version = &installer.config.igblast_version;
    let platform = if cfg!(target_os = "windows") {
        "x64-win64"
    } else if cfg!(target_os = "macos") {
        "x64-macosx"
    } else {
        "x64-linux"
    };
    let tarball = format!("ncbi-igblast-{version}-{platform}.tar.gz");
    let scratch = ScratchDir::new_in(installer.tools_root(), "igblast")?;
    let archive = scratch.path().join(&tarball);
    installer.download(&format!("{RELEASE_ROOT}/{version}/{tarball}"), &archive)?;
    installer.extract_archive(&archive, scratch.path())?;
    let unpacked = scratch.path().join(format!("ncbi-igblast-{version}"));
    let executable = unpacked.join("bin").join(if cfg!(target_os = "windows") {
        "igblastn.exe"
    } else {
        "igblastn"
    });
    if !executable.is_file() || !unpacked.join("internal_data").is_dir() {
        return Err(InstallError::InvalidConfiguration(format!(
            "the IgBLAST archive has an unexpected layout under {}",
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
            format!("unable to install IgBLAST into {}", target.display()),
            error,
        ));
    }
    let old_germline = old.join("germline_db");
    if old_germline.is_dir() && !target.join("germline_db").exists() {
        fs::rename(&old_germline, target.join("germline_db")).map_err(|error| {
            InstallError::io("unable to preserve the IgBLAST germline databases", error)
        })?;
    }
    fs::write(target.join(".version"), format!("{version}\n"))
        .map_err(|error| InstallError::io("unable to write the IgBLAST version marker", error))
}

fn install_databases(installer: &Installer, target: &Path) -> Result<(), InstallError> {
    let germline = target.join("germline_db");
    if directory_contains_extension(&germline, &["nhr", "phr"]) {
        installer.note("IgBLAST germline databases are already installed");
        return Ok(());
    }
    installer.step("Installing IgBLAST germline databases");
    fs::create_dir_all(&germline).map_err(|error| {
        InstallError::io(format!("unable to create {}", germline.display()), error)
    })?;
    let scratch = ScratchDir::new_in(installer.tools_root(), "igblast-db")?;
    for relative_url in DATABASE_ARCHIVES {
        let filename = relative_url.rsplit('/').next().unwrap_or("database.tar");
        let archive = scratch.path().join(filename);
        installer.download(&format!("{RELEASE_ROOT}/{relative_url}"), &archive)?;
        installer.extract_archive(&archive, &germline)?;
    }
    if !directory_contains_extension(&germline, &["nhr", "phr"]) {
        return Err(InstallError::InvalidConfiguration(format!(
            "no BLAST databases were extracted into {}",
            germline.display()
        )));
    }
    Ok(())
}
