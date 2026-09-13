//! Chai-1's out-of-band assets: Chai Discovery's official example MSAs, and Kalign.
//!
//! The two `.aligned.pqt` files are the MSAs from Chai-1's `examples/msas`, which the
//! `supplied_msas` preset folds against. At about 17 MiB they are far too large to embed in the
//! crate, the executable, or the Python wheel, so they are downloaded from the pinned upstream
//! revision when Chai-1 is installed, or the first time a run asks for them. They are distributed
//! by Chai Discovery under the Apache License 2.0:
//! <https://github.com/chaidiscovery/chai-lab/blob/main/LICENSE>
//!
//! Kalign is a separate C program, not a Python dependency, so `chai_lab` cannot pull it in: it
//! shells out to whatever `kalign` is on PATH and asserts if there is none. Every template
//! alignment goes through it, so building it here is what makes `--use-templates-server` and
//! `--template-hits-path` work on a machine where nobody ran `apt install kalign` by hand.

use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::Command,
};

use super::{InstallError, Installer, common::ScratchDir};

/// Where the MSAs are kept, relative to the tools root. Must match Chai-1's entry in
/// `Tool::asset_directories`, so uninstalling Chai-1 removes them.
const ASSET_DIRECTORY: &str = "chai1";

const SOURCE_URL: &str = "https://raw.githubusercontent.com/chaidiscovery/chai-lab/66c38d1fe5c6756a89ff8596b1dea87d305ec06f/examples/msas";

/// Each file is named by the SHA-256 of the protein sequence it aligns, which is how Chai-1's
/// `--msa-directory` finds it. The sizes are those of the files at the pinned revision.
const EXAMPLE_MSAS: [(&str, u64); 2] = [
    (
        "703adc2c74b8d7e613549b6efcf37126da7963522dc33852ad3c691eef1da06f.aligned.pqt",
        2_386_033,
    ),
    (
        "952a89ff052afbe8cd1656a317de8a4aa2457d6d73f50d228961bb84efd17e02.aligned.pqt",
        14_421_492,
    ),
];

/// Every Parquet file starts and ends with these four bytes.
const PARQUET_MAGIC: &[u8; 4] = b"PAR1";

/// The Kalign release to build. 3.4.0 is what Ubuntu packages and what `chai_lab`'s own error
/// message points at, so it is the version its template parsing has been exercised against.
const KALIGN_VERSION: &str = "3.4.0";

const KALIGN_SOURCE_URL: &str = "https://github.com/TimoLassmann/kalign/archive/refs/tags";

pub(super) fn directory(installer: &Installer) -> PathBuf {
    installer.tools_root().join(ASSET_DIRECTORY).join("msas")
}

/// Download whichever example MSAs are missing, and return the directory holding them.
pub(super) fn ensure_example_msas(installer: &Installer) -> Result<PathBuf, InstallError> {
    let directory = directory(installer);
    for (name, size) in EXAMPLE_MSAS {
        let path = directory.join(name);
        if is_complete(&path, size) {
            continue;
        }
        // `download` keeps any non-empty file it finds, so one of the wrong size has to go first.
        if path.exists() {
            fs::remove_file(&path).map_err(|error| {
                InstallError::io(format!("unable to replace {}", path.display()), error)
            })?;
        }
        let url = format!("{SOURCE_URL}/{name}");
        installer.download(&url, &path)?;
        if !is_complete(&path, size) {
            let _ = fs::remove_file(&path);
            return Err(InstallError::Download {
                url,
                message: format!("expected a {size}-byte Parquet file"),
            });
        }
    }
    Ok(directory)
}

/// A cheap integrity check: the exact upstream size, framed by Parquet's magic bytes.
fn is_complete(path: &Path, size: u64) -> bool {
    let check = || -> std::io::Result<bool> {
        let mut file = File::open(path)?;
        if file.metadata()?.len() != size {
            return Ok(false);
        }
        let mut head = [0; 4];
        let mut tail = [0; 4];
        file.read_exact(&mut head)?;
        file.seek(SeekFrom::End(-4))?;
        file.read_exact(&mut tail)?;
        Ok(&head == PARQUET_MAGIC && &tail == PARQUET_MAGIC)
    };
    check().unwrap_or(false)
}

/// The directory holding Kalign's binary. Chai-1 finds it by name rather than by path, so this is
/// what a run prepends to the child's PATH.
fn kalign_directory(installer: &Installer) -> PathBuf {
    kalign_prefix(installer).join("bin")
}

/// Build Kalign from source unless the wanted version is already installed, and return the
/// directory holding the binary.
pub(super) fn ensure_kalign(installer: &Installer) -> Result<PathBuf, InstallError> {
    let prefix = kalign_prefix(installer);
    let binary = prefix.join("bin").join("kalign");
    if installed_kalign_version(installer, &binary).as_deref() == Some(KALIGN_VERSION) {
        installer.note(format!("Kalign {KALIGN_VERSION} is already installed"));
        return Ok(kalign_directory(installer));
    }

    installer.step(format!("Building Kalign {KALIGN_VERSION}"));
    let scratch = ScratchDir::new_in(installer.tools_root(), "kalign")?;
    let archive = scratch
        .path()
        .join(format!("kalign-{KALIGN_VERSION}.tar.gz"));
    installer.download(
        &format!("{KALIGN_SOURCE_URL}/v{KALIGN_VERSION}.tar.gz"),
        &archive,
    )?;
    installer.extract_archive(&archive, scratch.path())?;
    let source = scratch.path().join(format!("kalign-{KALIGN_VERSION}"));
    let build = source.join("build");
    fs::create_dir_all(&build)
        .map_err(|error| InstallError::io("unable to create the Kalign build directory", error))?;

    let mut configure = Command::new("cmake");
    configure
        .arg("..")
        .arg("-DCMAKE_BUILD_TYPE=Release")
        // Kalign's default build puts the work in a shared libkalign that its binary then has to
        // find at run time. A static link keeps the binary self-contained, so a run can put it on
        // PATH without also arranging an LD_LIBRARY_PATH for it.
        .arg("-DBUILD_SHARED_LIBS=OFF")
        .arg(format!("-DCMAKE_INSTALL_PREFIX={}", prefix.display()))
        .current_dir(&build);
    installer.checked(&mut configure)?;
    let jobs = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .to_string();
    let mut build_command = Command::new("cmake");
    build_command
        .args(["--build", ".", "--parallel", &jobs])
        .current_dir(&build);
    installer.checked(&mut build_command)?;
    let mut install_command = Command::new("cmake");
    install_command.args(["--install", "."]).current_dir(&build);
    installer.checked(&mut install_command)?;

    if installed_kalign_version(installer, &binary).as_deref() != Some(KALIGN_VERSION) {
        return Err(InstallError::InvalidConfiguration(format!(
            "Kalign built, but {} does not report {KALIGN_VERSION}",
            binary.display()
        )));
    }
    installer.note(format!("Kalign installed at {}", binary.display()));
    Ok(kalign_directory(installer))
}

fn kalign_prefix(installer: &Installer) -> PathBuf {
    installer.tools_root().join(ASSET_DIRECTORY).join("kalign")
}

/// The version `binary` reports, or None if it is missing or does not run. Kalign prints
/// "kalign <version>", on stdout in 3.4 and on stderr in some builds, so both are searched.
fn installed_kalign_version(installer: &Installer, binary: &Path) -> Option<String> {
    if !binary.is_file() {
        return None;
    }
    let mut command = Command::new(binary);
    command.arg("--version");
    let output = installer.capture(&mut command).ok()?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    text.split_whitespace()
        .find(|word| word.starts_with(char::is_numeric))
        .map(str::to_owned)
}
