//! Chai Discovery's official example MSAs for Chai-1.
//!
//! These two `.aligned.pqt` files are the MSAs from Chai-1's `examples/msas`, which the
//! `supplied_msas` preset folds against. At about 17 MiB they are far too large to embed in the
//! crate, the executable, or the Python wheel, so they are downloaded from the pinned upstream
//! revision when Chai-1 is installed, or the first time a run asks for them. They are distributed
//! by Chai Discovery under the Apache License 2.0:
//! <https://github.com/chaidiscovery/chai-lab/blob/main/LICENSE>

use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

use super::{InstallError, Installer};

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

pub(super) fn directory(installer: &Installer) -> PathBuf {
    installer
        .tools_root()
        .join(ASSET_DIRECTORY)
        .join("msas")
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
