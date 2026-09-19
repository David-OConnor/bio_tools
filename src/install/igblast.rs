use std::{fs, path::Path, process::Command};

use super::{
    InstallError, Installer,
    common::{ScratchDir, directory_contains_extension},
};

const RELEASE_ROOT: &str = "https://ftp.ncbi.nih.gov/blast/executables/igblast/release";

/// Every germline database NCBI publishes prebuilt, and one name each set is
/// known by once it is unpacked, so a set that failed to download or that a
/// later release added is fetched rather than being covered by whatever else
/// is already in the directory.
///
/// These are all of them; the FTP listing under `database/` and `database/airr/`
/// holds nothing else. See
/// <https://ncbi.github.io/igblast/cook/How-to-set-up.html>.
const DATABASE_ARCHIVES: [(&str, &str); 5] = [
    // AIRR-C/OGRDB human IG: V, D (heavy) and J.
    ("database/airr/airr_c_human.tar", "airr_c_human_ig.V"),
    // AIRR-C/OGRDB mouse IG: one V set per inbred strain, and no D or J --
    // which is why a mouse run pairs these with the NCBI mouse D and J sets.
    ("database/airr/airr_c_mouse.tar", "airr_c_C57BL_6.V"),
    // NCBI's own mouse IG set: V, D and J.
    ("database/mouse_gl_VDJ.tar", "mouse_gl_V"),
    // NCBI's rhesus monkey IG set: V and J.
    ("database/rhesus_monkey_VJ.tar", "rhesus_monkey_V"),
    // Human constant-region genes, for `-c_region_db`.
    ("database/ncbi_human_c_genes.tar", "ncbi_human_c_genes"),
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

/// Download and unpack every prebuilt germline set that is not already there.
///
/// Each archive is checked for on its own, by a database it is known to
/// contain, rather than asking only whether the directory holds any BLAST
/// database at all: an install interrupted after the first archive, and a
/// release that adds a set, both left the old check satisfied and the
/// remaining organisms missing.
fn install_databases(installer: &Installer, target: &Path) -> Result<(), InstallError> {
    let germline = target.join("germline_db");
    let missing: Vec<_> = DATABASE_ARCHIVES
        .iter()
        .filter(|(_, present)| {
            // BLAST writes one header file per database, and a nucleotide set
            // and a protein set of the same name are two files.
            !germline.join(format!("{present}.nhr")).is_file()
                && !germline.join(format!("{present}.phr")).is_file()
        })
        .collect();
    if missing.is_empty() {
        installer.note("IgBLAST germline databases are already installed");
        return Ok(());
    }

    installer.step("Installing IgBLAST germline databases");
    fs::create_dir_all(&germline).map_err(|error| {
        InstallError::io(format!("unable to create {}", germline.display()), error)
    })?;
    let scratch = ScratchDir::new_in(installer.tools_root(), "igblast-db")?;
    for (relative_url, present) in missing {
        let filename = relative_url.rsplit('/').next().unwrap_or("database.tar");
        let archive = scratch.path().join(filename);
        installer.download(&format!("{RELEASE_ROOT}/{relative_url}"), &archive)?;
        installer.extract_archive(&archive, &germline)?;
        if !germline.join(format!("{present}.nhr")).is_file()
            && !germline.join(format!("{present}.phr")).is_file()
        {
            return Err(InstallError::InvalidConfiguration(format!(
                "{filename} did not unpack the {present} database into {}",
                germline.display()
            )));
        }
    }
    if !directory_contains_extension(&germline, &["nhr", "phr"]) {
        return Err(InstallError::InvalidConfiguration(format!(
            "no BLAST databases were extracted into {}",
            germline.display()
        )));
    }
    Ok(())
}
