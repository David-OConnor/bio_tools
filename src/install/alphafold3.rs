//! AlphaFold 3: the pinned source checkout, its environment, and (optionally) HMMER.
//!
//! What this installs unconditionally is the part Google publishes under Apache 2.0: the code at a
//! release tag, built into its own uv environment exactly as upstream's Dockerfile does it (`uv
//! sync --frozen` against the repository's lock file, then `build_data`). The model parameters are
//! a separate matter: Google publishes `af3.bin.zst` under the AlphaFold 3 Model Parameters Terms
//! of Use (non-commercial organisations only; no redistribution), and fetching it binds whoever
//! does so to those terms. So it is downloaded into [`models_dir`] only when the operator opts in
//! by setting `ALPHAFOLD3_ACCEPT_WEIGHTS_TERMS`; otherwise the install says where to put it. The
//! genetic databases (~630 GB unpacked) are never fetched: runs get their MSAs from the ColabFold
//! MMseqs2 server by default, and only the optional local data pipeline needs them.
//!
//! HMMER is built too, with upstream's `--seq_limit` patch to jackhmmer, because the local data
//! pipeline shells out to it. It is optional: a failed HMMER build is reported and the install
//! still succeeds, since the default ColabFold route never calls it.
//!
//! Installation guide: <https://github.com/google-deepmind/alphafold3/blob/v3.0.4/docs/installation.md>
//! Dockerfile: <https://github.com/google-deepmind/alphafold3/blob/v3.0.4/docker/Dockerfile>

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use super::{
    InstallError, Installer,
    common::{ScratchDir, scrub_python_environment},
};
use crate::tool_definitions::Tool;

const SLUG: &str = Tool::AlphaFold3.slug();

const REPOSITORY: &str = "https://github.com/google-deepmind/alphafold3.git";

/// The release tag to build. v3.0.4 is the first release whose inputs accept `description` (input
/// version 4), which is what the official `examples/` directory, and so the presets, are written
/// in.
pub(crate) const RELEASE: &str = "v3.0.4";

/// Everything AlphaFold 3 keeps outside its environment, relative to the tools root. The operator's
/// licensed parameters and any databases they add live here too, which is why uninstalling removes
/// only [`OWNED_DIRECTORIES`] rather than this whole directory.
pub(crate) const ASSET_DIRECTORY: &str = "alphafold3";

/// The subdirectories this recipe creates, and so may remove; see `Tool::asset_directories`.
pub(crate) const OWNED_DIRECTORIES: [&str; 3] = [
    "alphafold3/source",
    "alphafold3/hmmer",
    "alphafold3/jax_cache",
];

const HMMER_VERSION: &str = "3.4";
const HMMER_SOURCE_URL: &str = "http://eddylab.org/software/hmmer/hmmer-3.4.tar.gz";

/// The checkout, holding `run_alphafold.py`.
pub(crate) fn source_dir(installer: &Installer) -> PathBuf {
    installer.tools_root().join(ASSET_DIRECTORY).join("source")
}

/// Where the operator puts the model parameters (`af3.bin.zst`), unless `ALPHAFOLD3_MODEL_DIR`
/// names somewhere else.
pub(crate) fn models_dir(installer: &Installer) -> PathBuf {
    std::env::var_os("ALPHAFOLD3_MODEL_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| installer.tools_root().join(ASSET_DIRECTORY).join("models"))
}

/// Whether `directory` holds a file AlphaFold 3 will load as parameters. Mirrors the patterns
/// `alphafold3.model.params.select_model_files` accepts: `*.bin`, `*.bin.zst`, and their shards.
pub(crate) fn has_parameters(directory: &Path) -> bool {
    let Ok(entries) = fs::read_dir(directory) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let name = entry.file_name().to_string_lossy().into_owned();
        entry.path().is_file()
            && (name.ends_with(".bin")
                || name.ends_with(".bin.zst")
                || name
                    .rsplit_once(".bin.zst.")
                    .or_else(|| name.rsplit_once(".bin."))
                    .is_some_and(|(_, shard)| {
                        !shard.is_empty() && shard.chars().all(|c| c.is_ascii_digit())
                    }))
    })
}

pub(super) fn install(installer: &mut Installer) -> Result<(), InstallError> {
    let source = source_dir(installer);
    checkout_release(installer, &source)?;

    // Upstream's own recipe: a Python 3.12 environment synced from the repository's lock file.
    // Building the package compiles its C++ extension with CMake, which fetches its C++
    // dependencies itself; the host needs a C++ compiler, `make`, and zlib headers.
    installer.create_venv(SLUG, "3.12")?;
    let mut sync = installer.uv_command()?;
    sync.args(["sync", "--frozen", "--no-editable", "--no-dev"])
        .current_dir(&source);
    scrub_python_environment(&mut sync);
    sync.env("UV_PROJECT_ENVIRONMENT", installer.venv_dir(SLUG));
    if let Err(error) = installer.checked(&mut sync) {
        installer.note(
            "Building AlphaFold 3 needs git, a C++ compiler (gcc/g++), make, and zlib headers \
             (e.g. `apt install git gcc g++ make zlib1g-dev`).",
        );
        return Err(error);
    }

    // Converts the Chemical Component Dictionary shipped with the build into the pickles the
    // model reads at run time.
    let mut build_data = Command::new(installer.venv_script(SLUG, "build_data"));
    scrub_python_environment(&mut build_data);
    installer.checked(&mut build_data)?;

    let mut verify = Command::new(installer.venv_python(SLUG));
    verify.args([
        "-c",
        "import importlib.metadata as m, alphafold3.model.model, jax; \
         print('AlphaFold 3', m.version('alphafold3'), '| JAX devices:', jax.devices())",
    ]);
    scrub_python_environment(&mut verify);
    installer.checked(&mut verify)?;

    if let Err(error) = ensure_hmmer(installer, &source) {
        installer.note(format!(
            "HMMER was not built ({error}). It is only needed for AlphaFold 3's local data \
             pipeline; runs using the ColabFold MSA server or no MSAs are unaffected."
        ));
    }

    let models = models_dir(installer);
    fs::create_dir_all(&models).map_err(|error| {
        InstallError::io(format!("unable to create {}", models.display()), error)
    })?;
    if has_parameters(&models) {
        installer.note(format!(
            "Found AlphaFold 3 model parameters in {}",
            models.display()
        ));
    } else if weights_terms_accepted() {
        installer.note(format!(
            "{WEIGHTS_TERMS_VARIABLE} is set: downloading the model parameters from Google under \
             the AlphaFold 3 Model Parameters Terms of Use ({WEIGHTS_TERMS_URL})"
        ));
        installer.download(PARAMETERS_URL, &models.join("af3.bin.zst"))?;
    } else {
        installer.note(format!(
            "AlphaFold 3's model parameters are not installed. They are published by Google at \
             {PARAMETERS_URL} under the AlphaFold 3 Model Parameters Terms of Use \
             ({WEIGHTS_TERMS_URL}): non-commercial use by or on behalf of non-commercial \
             organisations only, and not to be shared outside your organisation. Once you have \
             read and accepted those terms, either download af3.bin.zst into {} (or point \
             ALPHAFOLD3_MODEL_DIR at the directory holding it), or set \
             {WEIGHTS_TERMS_VARIABLE}=1 and reinstall to have it downloaded for you.",
            models.display()
        ));
    }
    Ok(())
}

/// Where Google publishes the parameters. Fetched only on the operator's explicit acceptance of
/// the terms below; see [`weights_terms_accepted`].
const PARAMETERS_URL: &str = "https://storage.googleapis.com/alphafold3/af3.bin.zst";

const WEIGHTS_TERMS_URL: &str =
    "https://github.com/google-deepmind/alphafold3/blob/main/WEIGHTS_TERMS_OF_USE.md";

/// Set by an operator who has read and accepted the parameters' terms of use.
const WEIGHTS_TERMS_VARIABLE: &str = "ALPHAFOLD3_ACCEPT_WEIGHTS_TERMS";

/// Whether the operator has said they accept the AlphaFold 3 Model Parameters Terms of Use.
///
/// Downloading the parameters binds whoever does it to those terms, so an install never does it
/// on its own initiative: the operator opts in, once, by setting this variable.
fn weights_terms_accepted() -> bool {
    std::env::var(WEIGHTS_TERMS_VARIABLE).is_ok_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

/// Clone the repository if needed, and check out [`RELEASE`] detached.
fn checkout_release(installer: &Installer, source: &Path) -> Result<(), InstallError> {
    installer.clone_or_update(REPOSITORY, source)?;
    let mut fetch = Command::new("git");
    fetch
        .arg("-C")
        .arg(source)
        .args(["fetch", "--depth", "1", "origin", "tag", RELEASE]);
    installer.checked(&mut fetch)?;
    let mut checkout = Command::new("git");
    checkout
        .arg("-C")
        .arg(source)
        .args(["checkout", "--detach", RELEASE]);
    installer.checked(&mut checkout)
}

/// Where HMMER's binaries go. The adapter passes each one to `run_alphafold.py` by path.
pub(crate) fn hmmer_bin_dir(installer: &Installer) -> PathBuf {
    installer
        .tools_root()
        .join(ASSET_DIRECTORY)
        .join("hmmer")
        .join("bin")
}

/// Build HMMER with the patch upstream's Dockerfile applies, unless it is already built.
fn ensure_hmmer(installer: &Installer, source: &Path) -> Result<(), InstallError> {
    let bin = hmmer_bin_dir(installer);
    let jackhmmer = bin.join("jackhmmer");
    if jackhmmer.is_file() {
        installer.note(format!("HMMER is already installed at {}", bin.display()));
        return Ok(());
    }
    let prefix = bin.parent().unwrap().to_path_buf();

    installer.step(format!(
        "Building HMMER {HMMER_VERSION} for AlphaFold 3's data pipeline"
    ));
    let scratch = ScratchDir::new_in(installer.tools_root(), "hmmer")?;
    let archive = scratch.path().join(format!("hmmer-{HMMER_VERSION}.tar.gz"));
    installer.download(HMMER_SOURCE_URL, &archive)?;
    installer.extract_archive(&archive, scratch.path())?;

    // `--seq_limit` caps jackhmmer's hits, which `run_alphafold.py` passes; unpatched, every
    // protein search fails on an unknown option.
    let mut patch = Command::new("patch");
    patch
        .arg("-p0")
        .arg("-i")
        .arg(source.join("docker").join("jackhmmer_seq_limit.patch"))
        .current_dir(scratch.path());
    installer.checked(&mut patch)?;

    let build = scratch.path().join(format!("hmmer-{HMMER_VERSION}"));
    let mut configure = Command::new("./configure");
    configure
        .arg(format!("--prefix={}", prefix.display()))
        .current_dir(&build);
    installer.checked(&mut configure)?;
    let jobs = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .to_string();
    let mut make = Command::new("make");
    make.args(["-j", &jobs]).current_dir(&build);
    installer.checked(&mut make)?;
    let mut make_install = Command::new("make");
    make_install.arg("install").current_dir(&build);
    installer.checked(&mut make_install)?;
    // Easel's tools (esl-reformat and friends) are installed separately.
    let mut easel_install = Command::new("make");
    easel_install
        .arg("install")
        .current_dir(build.join("easel"));
    installer.checked(&mut easel_install)?;

    if !jackhmmer.is_file() {
        return Err(InstallError::InvalidConfiguration(format!(
            "HMMER built, but {} is missing",
            jackhmmer.display()
        )));
    }
    installer.note(format!("HMMER installed at {}", bin.display()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::has_parameters;

    #[test]
    fn parameter_files_match_what_alphafold_loads() {
        let directory =
            std::env::temp_dir().join(format!("bio-tools-af3-params-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        assert!(!has_parameters(&directory));

        std::fs::write(directory.join("README.txt"), "").unwrap();
        std::fs::write(directory.join("af3.bin.zst.partial"), "").unwrap();
        assert!(!has_parameters(&directory));

        for name in ["af3.bin.zst", "af3.bin", "af3.0.bin.zst", "af3.bin.zst.1"] {
            let path = directory.join(name);
            std::fs::write(&path, "").unwrap();
            assert!(has_parameters(&directory), "{name} should be recognised");
            std::fs::remove_file(path).unwrap();
        }
        std::fs::remove_dir_all(&directory).unwrap();
    }
}
