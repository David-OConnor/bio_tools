//! Tools whose dependencies come from the Conda package ecosystem rather than PyPI.
//!
//! Recipes we drive ourselves use micromamba. BindCraft and Genie 3 hand control to an upstream
//! `install.sh` that calls `conda info --base`, `conda shell.bash hook`, and `conda activate`, so
//! those two still bootstrap a full Miniconda via [`Installer::ensure_conda`].

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use super::{
    InstallError, Installer,
    common::{CONDA_FORGE, ScratchDir},
};
use crate::tool_definitions::Tool;

// `sokrypton/openfold` is an unmaintained fork whose setup.py asks CUDA 12 to
// compile retired architectures such as sm_37. This maintained OpenFold revision
// detects the installed GPU instead (including Ada's sm_89).
const GENIE3_OPENFOLD_URL: &str =
    "git+https://github.com/aqlaboratory/openfold.git@be2ec1841f16c966c65ae0e7599ebbadc725757d";
const LEGACY_GENIE3_OPENFOLD_URL: &str = "git+https://github.com/sokrypton/openfold.git";

// zhanggroup.org now sits behind a Cloudflare bot check that 403s wget's default User-Agent
// (curl with no UA gets the same 403; a browser-like UA passes), so the TMscore/TMalign
// downloads in Genie 3's own setup.sh need a UA override.
const BROWSER_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0 Safari/537.36";

// The ColabFold release HighFold's sources were forked from. Its pins -- AlphaFold 2.3.6,
// NumPy 1, TensorFlow for the feature pipeline -- are the stack that 2023 code expects, and it
// is the last release that still supports Python 3.10. The `-minus-jax` extra leaves JAX to the
// pin below, so the CUDA build is chosen rather than the CPU wheel.
const HIGHFOLD_COLABFOLD: &str = "colabfold[alphafold-minus-jax]==1.5.5";
// `jax.tree_map`, which HighFold's AlphaFold copy calls throughout, was removed in JAX 0.6, and
// 0.4 is the series the rest of these pins were resolved against. The CUDA compiler wheel needs
// its own ceiling: NVIDIA's 12.9 release dropped the `__init__.py` from `nvidia/cuda_nvcc`,
// making it an implicit namespace package whose `__file__` is None, and JAX 0.4.35 hands that
// straight to `pathlib.Path` while probing for a CUDA install. `import jax` then raises a
// TypeError. JAX's own floor here is 12.6.85, so this ceiling still satisfies it.
const HIGHFOLD_JAX: [&str; 2] = ["jax[cuda12]==0.4.35", "nvidia-cuda-nvcc-cu12==12.8.93"];
// Two of ColabFold 1.5.5's own pins no longer import: dm-haiku 0.0.10 reaches for
// `jax.linear_util`, which JAX 0.4.24 removed, and Biopython 1.82 dropped `Bio.Data.SCOPData`,
// which HighFold's AlphaFold copy imports. These go in afterwards, in their own pip run, because
// resolving them alongside ColabFold's metadata is a conflict rather than an override.
const HIGHFOLD_PIN_OVERRIDES: [&str; 2] = ["dm-haiku==0.0.13", "biopython==1.81"];

pub(super) fn install(installer: &mut Installer, tool: Tool) -> Result<(), InstallError> {
    match tool {
        Tool::HighFold => install_highfold(installer),
        Tool::BindCraft => install_bindcraft(installer),
        Tool::AntiFold => install_antifold(installer),
        Tool::Germinal => install_germinal(installer),
        Tool::Mber => install_mber(installer),
        Tool::Genie3 => install_genie3(installer),
        Tool::AggreScan3d => install_aggrescan3d(installer),
        _ => Err(InstallError::InvalidConfiguration(format!(
            "{} has no Conda recipe",
            tool.name()
        ))),
    }
}

fn install_highfold(installer: &mut Installer) -> Result<(), InstallError> {
    install_alphafold2_parameters(installer)?;
    let target = installer.tools_root().join("HighFold");
    installer.clone_or_update("https://github.com/hongliangduan/HighFold", &target)?;
    let prefix = installer.reset_mamba_environment(Tool::HighFold.slug(), "3.10")?;
    // ColabFold 1.5.5 caps NumPy below 2, so the Conda side has to resolve NumPy 1 builds up
    // front; left to itself it picks an OpenMM compiled against NumPy 2, which pip then breaks
    // when it downgrades NumPy underneath it.
    mamba_install(
        installer,
        &prefix,
        &["-c", CONDA_FORGE, "-c", "bioconda"],
        &["numpy<2", "openmm", "pdbfixer", "kalign2", "hhsuite"],
    )?;
    installer.mamba_run(
        &prefix,
        &["python", "-m", "pip", "install", HIGHFOLD_COLABFOLD],
    )?;
    installer.mamba_run(
        &prefix,
        &[&["python", "-m", "pip", "install"][..], &HIGHFOLD_JAX[..]].concat(),
    )?;
    for requirement in HIGHFOLD_PIN_OVERRIDES {
        installer.mamba_run(&prefix, &["python", "-m", "pip", "install", requirement])?;
    }
    overlay_highfold_sources(installer, &prefix, &target)?;

    let runner = installer.venv_script(Tool::HighFold.slug(), "colabfold_batch");
    if !runner.is_file() {
        return Err(InstallError::InvalidConfiguration(format!(
            "ColabFold installed without leaving a launcher at {}",
            runner.display()
        )));
    }
    Ok(())
}

/// Drop HighFold's sources over the installed ColabFold, which is upstream's own instruction:
/// the repository is a set of edited AlphaFold and ColabFold trees rather than a package, so
/// there is nothing here for pip to install. `utils/` stays behind -- it holds the paper's
/// evaluation scripts, and `batch.py` carries its own copy of the CycPOEM code they share.
fn overlay_highfold_sources(
    installer: &mut Installer,
    prefix: &Path,
    checkout: &Path,
) -> Result<(), InstallError> {
    let site_packages = environment_site_packages(installer, prefix)?;
    for package in ["alphafold", "colabfold"] {
        let source = checkout.join(package);
        if !source.is_dir() {
            return Err(InstallError::InvalidConfiguration(format!(
                "the HighFold checkout has no {package} directory at {}",
                source.display()
            )));
        }
        installer.step(format!("Overlaying HighFold's {package} sources"));
        overlay_directory(&source, &site_packages.join(package))?;
    }
    Ok(())
}

fn environment_site_packages(
    installer: &mut Installer,
    prefix: &Path,
) -> Result<PathBuf, InstallError> {
    let mut command = installer.micromamba_command()?;
    command.args(["run", "--prefix"]).arg(prefix).args([
        "python",
        "-c",
        "import sysconfig; print(sysconfig.get_paths()['purelib'])",
    ]);
    let output = installer.capture(&mut command)?;
    let path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    if !path.is_dir() {
        return Err(InstallError::InvalidConfiguration(format!(
            "{} has no site-packages directory",
            prefix.display()
        )));
    }
    Ok(path)
}

/// Copy `source` over `destination`, replacing files that exist in both and leaving the rest of
/// `destination` alone. Byte-compiled caches are skipped in both directions: the ones in the
/// checkout are whatever machine last ran it, and the ones already installed would shadow the
/// modules being replaced here.
fn overlay_directory(source: &Path, destination: &Path) -> Result<(), InstallError> {
    if destination
        .file_name()
        .is_some_and(|name| name == "__pycache__")
    {
        return Ok(());
    }
    fs::create_dir_all(destination).map_err(|error| {
        InstallError::io(format!("unable to create {}", destination.display()), error)
    })?;
    let stale = destination.join("__pycache__");
    if stale.is_dir() {
        fs::remove_dir_all(&stale).map_err(|error| {
            InstallError::io(format!("unable to clear {}", stale.display()), error)
        })?;
    }
    let entries = fs::read_dir(source)
        .map_err(|error| InstallError::io(format!("unable to read {}", source.display()), error))?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            InstallError::io(format!("unable to read {}", source.display()), error)
        })?;
        let name = entry.file_name();
        if name == "__pycache__" {
            continue;
        }
        let from = entry.path();
        let to = destination.join(&name);
        if from.is_dir() {
            overlay_directory(&from, &to)?;
        } else {
            fs::copy(&from, &to).map_err(|error| {
                InstallError::io(format!("unable to write {}", to.display()), error)
            })?;
        }
    }
    Ok(())
}

fn install_antifold(installer: &mut Installer) -> Result<(), InstallError> {
    let target = installer.tools_root().join("AntiFold");
    installer.clone_or_update("https://github.com/oxpig/AntiFold", &target)?;
    let prefix = installer.reset_mamba_environment(Tool::AntiFold.slug(), "3.10")?;
    // conda-forge skipped the 2.2 series entirely; under Conda this pin only resolved through the
    // implicit `defaults` channel. Upstream's own environment.yml sources Torch from `pytorch`.
    mamba_install(
        installer,
        &prefix,
        &["-c", "pytorch", "-c", CONDA_FORGE],
        &["pytorch==2.2.0"],
    )?;
    mamba_pip_install_path(installer, &prefix, &target, &[])
}

fn install_aggrescan3d(installer: &mut Installer) -> Result<(), InstallError> {
    let prefix = installer.reset_mamba_environment(Tool::AggreScan3d.slug(), "2.7")?;
    installer.mamba_run(
        &prefix,
        &[
            "python",
            "-m",
            "pip",
            "install",
            "git+https://bitbucket.org/lcbio/aggrescan3d.git@master",
        ],
    )
}

fn install_mber(installer: &mut Installer) -> Result<(), InstallError> {
    let target = installer.tools_root().join("mber-open");
    installer.clone_or_update("https://github.com/manifoldbio/mber-open", &target)?;
    let prefix = installer.venv_dir(Tool::Mber.slug());
    let mut remove = installer.micromamba_command()?;
    remove
        .args(["env", "remove", "--yes", "--prefix"])
        .arg(&prefix);
    let _ = installer.succeeds(&mut remove);
    if prefix.exists() {
        fs::remove_dir_all(&prefix).map_err(|error| {
            InstallError::io(
                format!("unable to clear the environment at {}", prefix.display()),
                error,
            )
        })?;
    }

    let source = fs::read_to_string(target.join("environment.yml"))
        .map_err(|error| InstallError::io("unable to read the mBER Conda environment", error))?;
    let conda_only = source
        .lines()
        .take_while(|line| {
            let line = line.trim_start();
            !line.starts_with("pip:") && !line.starts_with("- pip:")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let scratch = ScratchDir::new_in(installer.tools_root(), "mber-environment")?;
    let environment = scratch.path().join("environment.yml");
    fs::write(&environment, format!("{conda_only}\n")).map_err(|error| {
        InstallError::io("unable to write the mBER Conda-only environment", error)
    })?;
    let mut create = installer.micromamba_command()?;
    create
        .args(["create", "--yes", "--prefix"])
        .arg(&prefix)
        .arg("-f")
        .arg(environment)
        // ANARCI depends on HMMER. The newest HMMER build is MPI-enabled, which makes
        // micromamba copy Unix symlinks that DrvFS rejects when this layout is under /mnt/c.
        // Build 3 is the equivalent serial HMMER package and works on both native Linux and WSL.
        .arg("hmmer=3.4=*_3");
    installer.checked(&mut create)?;
    mamba_pip_install_path(
        installer,
        &prefix,
        &target,
        &[
            "--extra-index-url",
            "https://download.pytorch.org/whl/cu128",
            "-e",
        ],
    )?;
    mamba_pip_install_path(installer, &prefix, &target.join("protocols"), &["-e"])?;
    let download = target.join("download_weights.sh");
    installer.run_upstream_script(&download, &[], &target)
}

/// The named Conda environment a recipe creates, which [`Tool::conda_environment`] owns.
fn conda_environment(tool: Tool) -> &'static str {
    tool.conda_environment()
        .expect("this recipe creates a named Conda environment")
}

fn mamba_install(
    installer: &mut Installer,
    prefix: &Path,
    options: &[&str],
    packages: &[&str],
) -> Result<(), InstallError> {
    let mut command = installer.micromamba_command()?;
    command
        .arg("install")
        .arg("--prefix")
        .arg(prefix)
        .arg("--yes")
        .args(options)
        .args(packages);
    installer.checked(&mut command)
}

fn mamba_pip_install_path(
    installer: &mut Installer,
    prefix: &Path,
    package: &Path,
    extra_arguments: &[&str],
) -> Result<(), InstallError> {
    let mut command = installer.micromamba_command()?;
    command
        .args(["run", "--prefix"])
        .arg(prefix)
        .args(["python", "-m", "pip", "install"])
        .args(extra_arguments)
        .arg(package);
    installer.checked(&mut command)
}

/// The public AlphaFold 2 weights, laid out the way every consumer of them expects: AlphaFold
/// itself joins `params` onto whatever directory it is handed, so the weights sit one level
/// below the directory the adapters point at.
fn install_alphafold2_parameters(installer: &Installer) -> Result<(), InstallError> {
    let target = installer.tools_root().join("alphafold_params");
    let params = target.join("params");
    let marker = params.join("params_model_1_multimer_v3.npz");
    if marker.is_file() {
        installer.note("AlphaFold 2 parameters are already installed");
    } else if target.join("params_model_1_multimer_v3.npz").is_file() {
        // Earlier releases unpacked the archive flat into `alphafold_params`, where neither
        // ColabFold nor ColabDesign finds it. These are several GB, so move rather than refetch.
        installer.step("Moving the AlphaFold 2 parameters into their params/ subdirectory");
        move_directory_contents(&target, &params)?;
    } else {
        installer.step("Installing the public AlphaFold 2 parameters (several GB)");
        let scratch = ScratchDir::new_in(installer.tools_root(), "alphafold2-params")?;
        let archive = scratch.path().join("params.tar");
        installer.download(
            "https://storage.googleapis.com/alphafold/alphafold_params_2022-12-06.tar",
            &archive,
        )?;
        installer.extract_archive(&archive, &params)?;
    }
    if !marker.is_file() {
        return Err(InstallError::InvalidConfiguration(format!(
            "the AlphaFold 2 parameters did not unpack into {}",
            params.display()
        )));
    }
    // ColabFold checks for these before deciding whether to fetch its own copy of the same
    // weights, which is another several-GB download over a directory that already has them.
    for name in [
        "download_finished.txt",
        "download_complexes_multimer_v3_finished.txt",
    ] {
        let flag = params.join(name);
        if !flag.exists() {
            fs::write(&flag, "").map_err(|error| {
                InstallError::io(format!("unable to write {}", flag.display()), error)
            })?;
        }
    }
    Ok(())
}

fn move_directory_contents(source: &Path, destination: &Path) -> Result<(), InstallError> {
    fs::create_dir_all(destination).map_err(|error| {
        InstallError::io(format!("unable to create {}", destination.display()), error)
    })?;
    let entries = fs::read_dir(source)
        .map_err(|error| InstallError::io(format!("unable to read {}", source.display()), error))?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            InstallError::io(format!("unable to read {}", source.display()), error)
        })?;
        let from = entry.path();
        if from == destination {
            continue;
        }
        let to = destination.join(entry.file_name());
        fs::rename(&from, &to).map_err(|error| {
            InstallError::io(
                format!("unable to move {} to {}", from.display(), to.display()),
                error,
            )
        })?;
    }
    Ok(())
}

/// Stays on full Conda: `install_bindcraft.sh` resolves `conda info --base` and then sources
/// `$CONDA_BASE/bin/activate`, neither of which exists in a micromamba root.
fn install_bindcraft(installer: &mut Installer) -> Result<(), InstallError> {
    let environment = conda_environment(Tool::BindCraft);
    let target = installer.tools_root().join("BindCraft");
    let marker = target.join("params/params_model_5_ptm.npz");
    let conda = installer.ensure_conda()?;
    let mut probe = Command::new(&conda);
    probe.args(["run", "--name", environment, "python", "--version"]);
    if marker.is_file() && installer.succeeds(&mut probe) {
        installer.install_conda_environment_shims(Tool::BindCraft)?;
        installer.note("BindCraft is already installed");
        return Ok(());
    }
    installer.clone_or_update("https://github.com/martinpacesa/BindCraft", &target)?;
    let mut remove = Command::new(&conda);
    remove.args(["env", "remove", "--name", environment, "-y"]);
    let _ = installer.succeeds(&mut remove);

    let cuda = env::var("BINDCRAFT_CUDA").unwrap_or_else(|_| "12.4".to_owned());
    run_bash_with_conda(
        installer,
        &target.join("install_bindcraft.sh"),
        &["--cuda", &cuda, "--pkg_manager", "conda"],
        &target,
    )?;
    if !marker.is_file() {
        return Err(InstallError::InvalidConfiguration(format!(
            "BindCraft completed without creating {}",
            marker.display()
        )));
    }
    installer.install_conda_environment_shims(Tool::BindCraft)
}

fn install_germinal(installer: &mut Installer) -> Result<(), InstallError> {
    let target = installer.tools_root().join("germinal");
    installer.clone_or_update("https://github.com/SantiagoMille/germinal", &target)?;
    // Upstream currently ships only an environment.yml, so the prefix branch below is what runs.
    // The script branch is kept because earlier releases did carry an install.sh.
    let upstream = target.join("install.sh");
    if upstream.is_file() {
        let conda = installer.ensure_conda()?;
        let mut remove = Command::new(&conda);
        remove.args([
            "env",
            "remove",
            "--name",
            conda_environment(Tool::Germinal),
            "-y",
        ]);
        let _ = installer.succeeds(&mut remove);
        return run_bash_with_conda(installer, &upstream, &[], &target);
    }
    let prefix = installer.reset_mamba_environment(Tool::Germinal.slug(), "3.11")?;
    mamba_pip_install_path(installer, &prefix, &target, &[])
}

/// Stays on full Conda: `scripts/setup/setup.sh` runs `eval "$(conda shell.bash hook)"` followed by
/// `conda activate`, and micromamba's hook takes a different form.
fn install_genie3(installer: &mut Installer) -> Result<(), InstallError> {
    let environment = conda_environment(Tool::Genie3);
    let target = installer.tools_root().join("genie3");
    let conda = installer.ensure_conda()?;
    let mut existing = Command::new(&conda);
    existing.args([
        "run",
        "--name",
        environment,
        "python",
        "-c",
        "import torch; assert torch.cuda.is_available()",
    ]);
    if target.join("pretrained").is_dir() && installer.succeeds(&mut existing) {
        installer.install_conda_environment_shims(Tool::Genie3)?;
        installer.note("Genie 3 is already installed");
        return Ok(());
    }
    if installer.succeeds(&mut existing) {
        installer.note("Genie 3 environment is ready; downloading missing model weights");
        run_bash_in_conda_environment(
            installer,
            environment,
            &target.join("scripts/setup/download.sh"),
            &["--weights"],
            &target,
        )?;
        return installer.install_conda_environment_shims(Tool::Genie3);
    }
    installer.clone_or_update("https://github.com/aqlaboratory/genie3", &target)?;
    let mut remove = Command::new(&conda);
    remove.args(["env", "remove", "--name", environment, "-y"]);
    let _ = installer.succeeds(&mut remove);
    let mut create = Command::new(&conda);
    create.args(["create", "--name", environment, "python=3.10", "-y"]);
    installer.checked(&mut create)?;

    let cuda = env::var("GENIE3_NVCC_CUDA").unwrap_or_else(|_| "12.4.1".to_owned());
    let mut nvcc = Command::new(&conda);
    nvcc.args([
        "install",
        "--name",
        environment,
        "-y",
        "-c",
        &format!("nvidia/label/cuda-{cuda}"),
        "cuda-toolkit",
    ]);
    installer.checked(&mut nvcc)?;
    patch_genie3_openfold_source(&target.join("scripts/setup/setup.sh"))?;
    patch_genie3_zhanggroup_downloads(&target.join("scripts/setup/setup.sh"))?;
    run_bash_with_conda(
        installer,
        &target.join("scripts/setup/setup.sh"),
        &[],
        &target,
    )?;
    run_bash_in_conda_environment(
        installer,
        environment,
        &target.join("scripts/setup/download.sh"),
        &["--weights"],
        &target,
    )?;
    let mut verify = Command::new(conda);
    verify.args([
        "run",
        "--name",
        environment,
        "python",
        "-c",
        "import torch; assert torch.cuda.is_available(), 'Genie 3 requires CUDA'",
    ]);
    installer.checked(&mut verify)?;
    installer.install_conda_environment_shims(Tool::Genie3)
}

fn patch_genie3_openfold_source(script: &Path) -> Result<(), InstallError> {
    let source = fs::read_to_string(script).map_err(|error| {
        InstallError::InvalidConfiguration(format!(
            "unable to read Genie 3 setup script {}: {error}",
            script.display()
        ))
    })?;
    if source.contains(GENIE3_OPENFOLD_URL) {
        return Ok(());
    }
    let updated = source.replace(LEGACY_GENIE3_OPENFOLD_URL, GENIE3_OPENFOLD_URL);
    if updated == source {
        return Err(InstallError::InvalidConfiguration(format!(
            "Genie 3 setup script {} no longer contains its expected OpenFold source",
            script.display()
        )));
    }
    fs::write(script, updated).map_err(|error| {
        InstallError::InvalidConfiguration(format!(
            "unable to update Genie 3 setup script {}: {error}",
            script.display()
        ))
    })
}

fn patch_genie3_zhanggroup_downloads(script: &Path) -> Result<(), InstallError> {
    let source = fs::read_to_string(script).map_err(|error| {
        InstallError::InvalidConfiguration(format!(
            "unable to read Genie 3 setup script {}: {error}",
            script.display()
        ))
    })?;
    let needle = "wget https://zhanggroup.org";
    let replacement = format!(r#"wget --user-agent="{BROWSER_USER_AGENT}" https://zhanggroup.org"#);
    if !source.contains(needle) {
        // Already patched, or upstream changed its download command; either way there is
        // nothing more for this patch to do.
        return Ok(());
    }
    let updated = source.replace(needle, &replacement);
    fs::write(script, updated).map_err(|error| {
        InstallError::InvalidConfiguration(format!(
            "unable to update Genie 3 setup script {}: {error}",
            script.display()
        ))
    })
}

fn run_bash_with_conda(
    installer: &mut Installer,
    script: &Path,
    arguments: &[&str],
    cwd: &Path,
) -> Result<(), InstallError> {
    if !script.is_file() {
        return Err(InstallError::InvalidConfiguration(format!(
            "upstream installer {} was not found",
            script.display()
        )));
    }
    let conda = installer.ensure_conda()?;
    let mut command = Command::new("bash");
    command.arg(script).args(arguments).current_dir(cwd);
    if let Some(directory) = conda.parent()
        && let Some(existing) = env::var_os("PATH")
    {
        let paths = std::iter::once(directory.to_path_buf()).chain(env::split_paths(&existing));
        let joined = env::join_paths(paths).map_err(|error| {
            InstallError::InvalidConfiguration(format!(
                "unable to add Conda to the upstream installer's PATH: {error}"
            ))
        })?;
        command.env("PATH", joined);
    }
    installer.checked(&mut command)
}

fn run_bash_in_conda_environment(
    installer: &mut Installer,
    environment: &str,
    script: &Path,
    arguments: &[&str],
    cwd: &Path,
) -> Result<(), InstallError> {
    if !script.is_file() {
        return Err(InstallError::InvalidConfiguration(format!(
            "upstream installer {} was not found",
            script.display()
        )));
    }
    let conda = installer.ensure_conda()?;
    let mut command = Command::new(conda);
    command
        .args(["run", "--name", environment, "bash"])
        .arg(script)
        .args(arguments)
        .current_dir(cwd);
    installer.checked(&mut command)
}
