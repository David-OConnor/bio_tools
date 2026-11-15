use std::{
    env,
    path::{self, PathBuf},
};

use super::{
    InstallError, Installer,
    common::{CONDA_FORGE, ScratchDir},
};
use crate::{
    run::{self, CommandSpec},
    tool_definitions::Tool,
};

const TOOL: Tool = Tool::Psi4;
const PSI4: &str = "psi4==1.11";
const PYTHON: &str = "python=3.15";

pub(super) fn install(installer: &mut Installer) -> Result<(), InstallError> {
    let prefix = installer.environment_path(TOOL);
    let action = if prefix.join("conda-meta/history").is_file() {
        "install"
    } else {
        "create"
    };
    let mut command = installer.micromamba_command()?;
    command
        .args([action, "--yes", "--prefix"])
        .arg(prefix)
        .args(["--override-channels", "-c", CONDA_FORGE, PSI4, PYTHON]);
    if cfg!(all(windows, target_arch = "aarch64")) {
        // Psi4 publishes only win-64. Windows 11 on ARM can run it via x64 emulation.
        command.env("CONDA_SUBDIR", "win-64");
    }
    installer.checked(&mut command)?;

    run::run(&installer.tool_command(TOOL).arg("--version")).map_err(|error| {
        InstallError::InvalidConfiguration(format!("Psi4 probe failed: {error}"))
    })?;
    installer.note(probe(installer)?);
    Ok(())
}

pub(crate) fn probe_command(installer: &Installer) -> CommandSpec {
    python_command(installer).args(["-c", "import psi4; print('Psi4', psi4.__version__)"])
}

pub(crate) fn probe(installer: &Installer) -> Result<String, InstallError> {
    // Importing Psi4 writes timer.dat at exit.
    let scratch = ScratchDir::new_in(&env::temp_dir(), "psi4-probe")?;
    let output =
        run::run(&probe_command(installer).current_dir(scratch.path())).map_err(|error| {
            InstallError::InvalidConfiguration(format!("Psi4 probe failed: {error}"))
        })?;
    Ok(output.stdout_lossy().trim().to_owned())
}

pub(super) fn launcher(installer: &Installer) -> PathBuf {
    let filename = if cfg!(windows) {
        "psi4-script.py"
    } else {
        "psi4"
    };
    let launcher = installer.venv_scripts_dir(TOOL.slug()).join(filename);
    path::absolute(&launcher).unwrap_or(launcher)
}

pub(super) fn python_command(installer: &Installer) -> CommandSpec {
    let prefix = installer.environment_path(TOOL);
    let prefix = path::absolute(&prefix).unwrap_or(prefix);
    let (python, mut paths) = if cfg!(windows) {
        (
            prefix.join("python.exe"),
            vec![
                prefix.clone(),
                prefix.join("Library/bin"),
                prefix.join("Scripts"),
            ],
        )
    } else {
        let scripts = prefix.join("bin");
        (scripts.join("python"), vec![scripts])
    };
    if let Some(inherited) = env::var_os("PATH") {
        paths.extend(env::split_paths(&inherited));
    }
    let command = CommandSpec::new(python)
        .arg("-I")
        .env("CONDA_PREFIX", prefix);
    match env::join_paths(paths) {
        Ok(path) => command.env("PATH", path),
        Err(_) => command,
    }
}
