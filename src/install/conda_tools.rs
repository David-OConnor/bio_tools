use std::{env, fs, path::Path, process::Command};

use super::{InstallError, Installer, Tool, common::ScratchDir};

pub(super) fn install(installer: &mut Installer, tool: Tool) -> Result<(), InstallError> {
    prepare_conda(installer)?;
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

fn prepare_conda(installer: &mut Installer) -> Result<(), InstallError> {
    let conda = installer.ensure_conda()?;
    // Conda 24.9+ requires explicit acceptance for defaults in non-interactive installs. Older
    // versions do not expose `conda tos`, so absence of that subcommand is harmless.
    for channel in [
        "https://repo.anaconda.com/pkgs/main",
        "https://repo.anaconda.com/pkgs/r",
    ] {
        let mut command = Command::new(&conda);
        command.args(["tos", "accept", "--override-channels", "--channel", channel]);
        let _ = installer.succeeds(&mut command);
    }
    Ok(())
}

fn install_highfold(installer: &mut Installer) -> Result<(), InstallError> {
    install_alphafold2_parameters(installer)?;
    let target = installer.tools_root().join("HighFold");
    installer.clone_or_update("https://github.com/hongliangduan/HighFold", &target)?;
    let prefix = installer.reset_conda_environment("highfold", "3.10")?;
    conda_install(
        installer,
        &prefix,
        &["-c", "conda-forge", "-c", "bioconda"],
        &["openmm", "pdbfixer", "kalign2", "hhsuite"],
    )?;
    installer.conda_run(
        &prefix,
        &["python", "-m", "pip", "install", "--upgrade", "jax[cuda12]"],
    )?;
    conda_pip_install_path(installer, &prefix, &target, &[])
}

fn install_antifold(installer: &mut Installer) -> Result<(), InstallError> {
    let target = installer.tools_root().join("AntiFold");
    installer.clone_or_update("https://github.com/oxpig/AntiFold", &target)?;
    let prefix = installer.reset_conda_environment("antifold", "3.10")?;
    conda_install(
        installer,
        &prefix,
        &["-c", "conda-forge"],
        &["pytorch==2.2.0"],
    )?;
    conda_pip_install_path(installer, &prefix, &target, &[])
}

fn install_aggrescan3d(installer: &mut Installer) -> Result<(), InstallError> {
    let prefix = installer.reset_conda_environment("aggrescan3d", "2.7")?;
    installer.conda_run(
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
    let conda = installer.ensure_conda()?;
    let prefix = installer.venv_dir("mber");
    let mut remove = Command::new(&conda);
    remove
        .args(["env", "remove", "--prefix"])
        .arg(&prefix)
        .arg("-y");
    let _ = installer.succeeds(&mut remove);

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
    let mut create = Command::new(conda);
    create
        .args(["env", "create", "--prefix"])
        .arg(&prefix)
        .arg("-f")
        .arg(environment);
    installer.checked(&mut create)?;
    conda_pip_install_path(
        installer,
        &prefix,
        &target,
        &[
            "-e",
            "--extra-index-url",
            "https://download.pytorch.org/whl/cu128",
        ],
    )?;
    conda_pip_install_path(installer, &prefix, &target.join("protocols"), &["-e"])?;
    let download = target.join("download_weights.sh");
    installer.run_upstream_script(&download, &[], &target)
}

fn conda_install(
    installer: &mut Installer,
    prefix: &Path,
    options: &[&str],
    packages: &[&str],
) -> Result<(), InstallError> {
    let conda = installer.ensure_conda()?;
    let mut command = Command::new(conda);
    command
        .arg("install")
        .arg("--prefix")
        .arg(prefix)
        .arg("-y")
        .args(options)
        .args(packages);
    installer.checked(&mut command)
}

fn conda_pip_install_path(
    installer: &mut Installer,
    prefix: &Path,
    package: &Path,
    extra_arguments: &[&str],
) -> Result<(), InstallError> {
    let conda = installer.ensure_conda()?;
    let mut command = Command::new(conda);
    command
        .args(["run", "--prefix"])
        .arg(prefix)
        .args(["python", "-m", "pip", "install"])
        .args(extra_arguments)
        .arg(package);
    installer.checked(&mut command)
}

fn install_alphafold2_parameters(installer: &Installer) -> Result<(), InstallError> {
    let target = installer.tools_root().join("alphafold_params");
    if target.join("params_model_1_multimer_v3.npz").is_file() {
        installer.note("AlphaFold 2 parameters are already installed");
        return Ok(());
    }
    installer.step("Installing the public AlphaFold 2 parameters (several GB)");
    let scratch = ScratchDir::new_in(installer.tools_root(), "alphafold2-params")?;
    let archive = scratch.path().join("params.tar");
    installer.download(
        "https://storage.googleapis.com/alphafold/alphafold_params_2022-12-06.tar",
        &archive,
    )?;
    installer.extract_archive(&archive, &target)?;
    if !target.join("params_model_1_multimer_v3.npz").is_file() {
        return Err(InstallError::InvalidConfiguration(format!(
            "the AlphaFold 2 parameters did not unpack into {}",
            target.display()
        )));
    }
    Ok(())
}

fn install_bindcraft(installer: &mut Installer) -> Result<(), InstallError> {
    let target = installer.tools_root().join("BindCraft");
    let marker = target.join("params/params_model_5_ptm.npz");
    let conda = installer.ensure_conda()?;
    let mut probe = Command::new(&conda);
    probe.args(["run", "--name", "BindCraft", "python", "--version"]);
    if marker.is_file() && installer.succeeds(&mut probe) {
        installer.note("BindCraft is already installed");
        return Ok(());
    }
    installer.clone_or_update("https://github.com/martinpacesa/BindCraft", &target)?;
    let mut remove = Command::new(&conda);
    remove.args(["env", "remove", "--name", "BindCraft", "-y"]);
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
    Ok(())
}

fn install_germinal(installer: &mut Installer) -> Result<(), InstallError> {
    let target = installer.tools_root().join("germinal");
    installer.clone_or_update("https://github.com/SantiagoMille/germinal", &target)?;
    let conda = installer.ensure_conda()?;
    let mut remove = Command::new(&conda);
    remove.args(["env", "remove", "--name", "germinal", "-y"]);
    let _ = installer.succeeds(&mut remove);
    let upstream = target.join("install.sh");
    if upstream.is_file() {
        run_bash_with_conda(installer, &upstream, &[], &target)
    } else {
        let prefix = installer.reset_conda_environment("germinal", "3.11")?;
        conda_pip_install_path(installer, &prefix, &target, &[])
    }
}

fn install_genie3(installer: &mut Installer) -> Result<(), InstallError> {
    let target = installer.tools_root().join("genie3");
    let conda = installer.ensure_conda()?;
    let mut existing = Command::new(&conda);
    existing.args([
        "run",
        "--name",
        "genie3",
        "python",
        "-c",
        "import torch; assert torch.cuda.is_available()",
    ]);
    if target.join("pretrained").is_dir() && installer.succeeds(&mut existing) {
        installer.note("Genie 3 is already installed");
        return Ok(());
    }
    installer.clone_or_update("https://github.com/aqlaboratory/genie3", &target)?;
    let mut remove = Command::new(&conda);
    remove.args(["env", "remove", "--name", "genie3", "-y"]);
    let _ = installer.succeeds(&mut remove);
    let mut create = Command::new(&conda);
    create.args(["create", "--name", "genie3", "python=3.10", "-y"]);
    installer.checked(&mut create)?;

    let cuda = env::var("GENIE3_NVCC_CUDA").unwrap_or_else(|_| "12.4.1".to_owned());
    let mut nvcc = Command::new(&conda);
    nvcc.args([
        "install",
        "--name",
        "genie3",
        "-y",
        "-c",
        &format!("nvidia/label/cuda-{cuda}"),
        "cuda-toolkit",
    ]);
    installer.checked(&mut nvcc)?;
    run_bash_with_conda(
        installer,
        &target.join("scripts/setup/setup.sh"),
        &[],
        &target,
    )?;
    run_bash_with_conda(
        installer,
        &target.join("scripts/setup/download.sh"),
        &["--weights"],
        &target,
    )?;
    let mut verify = Command::new(conda);
    verify.args([
        "run",
        "--name",
        "genie3",
        "python",
        "-c",
        "import torch; assert torch.cuda.is_available(), 'Genie 3 requires CUDA'",
    ]);
    installer.checked(&mut verify)
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
