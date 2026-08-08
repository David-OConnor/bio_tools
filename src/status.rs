//! Get the status of each tool by running it with a simple command like `--version`, and confirming
//! the application and exists and is healthy, or if there's a problem. On a tool-by-tool basis,
//! provies additional data like computation devices configured for.

use std::{env, fs, path::PathBuf, time::Duration};

pub use crate::install::{StatusKind, ToolStatus};
use crate::{
    install::{InstallError, Installer},
    run::{CaptureLimits, CommandOutput, CommandSpec, ExitPolicy, RunError},
    tool_definitions::Tool,
};

const STATUS_DIRECTORY: &str = ".bio_tools";

pub(crate) fn record_install(installer: &Installer, tool: Tool) -> Result<(), InstallError> {
    let directory = installer
        .config
        .layout
        .environments_root
        .join(STATUS_DIRECTORY);

    fs::create_dir_all(&directory).map_err(|error| {
        InstallError::io(
            format!("unable to create status directory {}", directory.display()),
            error,
        )
    })?;

    let marker = directory.join(format!("{}.installed", tool.slug()));
    fs::write(&marker, format!("{}\n", tool.slug())).map_err(|error| {
        InstallError::io(
            format!("unable to write installation marker {}", marker.display()),
            error,
        )
    })
}

pub(crate) fn forget_install(installer: &Installer, tool: Tool) -> Result<(), InstallError> {
    let marker = marker_path(installer, tool);
    match fs::remove_file(&marker) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(InstallError::io(
            format!(
                "unable to remove the installation marker {}",
                marker.display()
            ),
            error,
        )),
    }
}

fn marker_path(installer: &Installer, tool: Tool) -> PathBuf {
    installer
        .config
        .layout
        .environments_root
        .join(STATUS_DIRECTORY)
        .join(format!("{}.installed", tool.slug()))
}

/// Quickly inspect every tool installed by this crate without launching its application code.
///
/// The returned order is the stable, user-facing [`Tool::ALL`] order.
pub fn list_quick(installer: &Installer) -> Vec<(Tool, ToolStatus)> {
    Tool::ALL
        .into_iter()
        .filter(|tool| marker_path(installer, *tool).is_file())
        .map(|tool| (tool, status_quick(installer, tool)))
        .collect()
}

/// Fully probe every tool installed by this crate using the supplied installation layout.
///
/// Unlike [`list_quick`], this launches each application and, where applicable, imports its
/// compute framework to determine whether its environment can use a GPU.
pub fn list_full(installer: &Installer) -> Vec<(Tool, ToolStatus)> {
    Tool::ALL
        .into_iter()
        .filter(|tool| marker_path(installer, *tool).is_file())
        .map(|tool| (tool, status_full(installer, tool)))
        .collect()
}

/// Backwards-compatible alias for [`list_full`].
pub fn list(installer: &Installer) -> Vec<(Tool, ToolStatus)> {
    list_full(installer)
}

/// Check installation markers, required assets, and the probe executable without launching the
/// tool. This is intended for interactive status pages and other latency-sensitive callers.
pub fn status_quick(installer: &Installer, tool: Tool) -> ToolStatus {
    let was_installed = match preliminary_check(installer, tool) {
        Ok(was_installed) => was_installed,
        Err(status) => return status,
    };

    let command = if tool == Tool::AlphaFold3 {
        match alphafold3_command(installer, was_installed) {
            Ok(command) => Some(command),
            Err(status) => return status,
        }
    } else {
        probe_command(installer, tool)
    };

    let Some(command) = command else {
        return if was_installed {
            pass(
                "The bio_tools installation recipe completed successfully.",
                None,
            )
        } else {
            not_found(format!(
                "No bio_tools installation marker was found for {}.",
                tool.name()
            ))
        };
    };

    let executable = PathBuf::from(command.program.as_os_str());
    if !executable.is_file() {
        return missing_or_broken(
            was_installed,
            format!("Missing status executable at {}.", executable.display()),
        );
    }

    // A named Conda probe starts the shared Conda executable, not a file inside the tool's
    // environment. Its install marker and required assets are the quick evidence that the
    // environment recipe completed; without that marker, finding Conda says nothing about the
    // particular tool.
    if (matches!(tool, Tool::BindCraft | Tool::Genie3)
        || (tool == Tool::Germinal && !installer.venv_python(tool.slug()).is_file()))
        && !was_installed
    {
        return not_found(format!(
            "No bio_tools installation marker was found for {}.",
            tool.name()
        ));
    }

    pass(
        format!(
            "Found {}'s executable and required installation files.",
            tool.name()
        ),
        None,
    )
}

/// Launch a tool with its help/version probe and inspect its compute device where applicable.
pub fn status_full(installer: &Installer, tool: Tool) -> ToolStatus {
    let was_installed = match preliminary_check(installer, tool) {
        Ok(was_installed) => was_installed,
        Err(status) => return status,
    };

    if tool == Tool::AlphaFold3 {
        return check_alphafold3(installer, was_installed);
    }

    let command = match probe_command(installer, tool) {
        Some(command) => command,
        None if was_installed => {
            return pass(
                "The bio_tools installation recipe completed successfully.",
                None,
            );
        }
        None => {
            return not_found(format!(
                "No bio_tools installation marker was found for {}.",
                tool.name()
            ));
        }
    };

    let output = match run_probe(command) {
        Ok(output) => output,
        Err(cause) if cause.io_error_kind() == Some(std::io::ErrorKind::NotFound) => {
            return missing_or_broken(was_installed, cause.to_string());
        }
        Err(cause) => return error(format!("Could not probe {}: {cause}", tool.name())),
    };
    let detail = output_detail(&output);
    if !output.status.success()
        && (detail.is_empty()
            || detail
                .trim_start()
                .starts_with("Traceback (most recent call last):"))
    {
        return error(if detail.is_empty() {
            format!("The {} status probe exited unsuccessfully.", tool.name())
        } else {
            detail
        });
    }

    let detail = if detail.is_empty() {
        format!("{} answered its status probe.", tool.name())
    } else {
        detail
            .lines()
            .next()
            .unwrap_or_default()
            .chars()
            .take(200)
            .collect()
    };
    pass(detail, probe_device(installer, tool))
}

fn preliminary_check(installer: &Installer, tool: Tool) -> Result<bool, ToolStatus> {
    if !tool.is_supported() {
        return Err(error(format!(
            "{} is not supported on this operating system.",
            tool.name()
        )));
    }

    let was_installed = marker_path(installer, tool).is_file();

    for (path, description) in required_paths(installer, tool) {
        if !path.exists() {
            return Err(missing_or_broken(
                was_installed,
                format!("Missing {description} at {}.", path.display()),
            ));
        }
    }
    Ok(was_installed)
}

fn check_alphafold3(installer: &Installer, was_installed: bool) -> ToolStatus {
    let command = match alphafold3_command(installer, was_installed) {
        Ok(command) => command,
        Err(status) => return status,
    };
    match run_probe(command) {
        Ok(output) if output.status.success() || !output_detail(&output).is_empty() => pass(
            output_detail(&output)
                .lines()
                .next()
                .unwrap_or("AlphaFold 3 answered its status probe."),
            probe_device(installer, Tool::AlphaFold3),
        ),
        Ok(output) => error(format!(
            "AlphaFold 3 did not answer its status probe: {}",
            output_detail(&output)
        )),
        Err(cause) => missing_or_broken(was_installed, cause.to_string()),
    }
}

fn alphafold3_command(
    installer: &Installer,
    was_installed: bool,
) -> Result<CommandSpec, ToolStatus> {
    let Some(runner) = env::var_os("ALPHAFOLD3_RUNNER").map(PathBuf::from) else {
        return Err(missing_or_broken(
            was_installed,
            "The Python environment is prepared, but ALPHAFOLD3_RUNNER is not configured."
                .to_owned(),
        ));
    };
    for (name, variable) in [
        ("model directory", "ALPHAFOLD3_MODEL_DIR"),
        ("database directory", "ALPHAFOLD3_DATABASE_DIR"),
    ] {
        let Some(path) = env::var_os(variable).map(PathBuf::from) else {
            return Err(missing_or_broken(
                was_installed,
                format!("{variable} is not configured."),
            ));
        };
        if !path.exists() {
            return Err(missing_or_broken(
                was_installed,
                format!(
                    "The configured {name} does not exist at {}.",
                    path.display()
                ),
            ));
        }
    }
    if !runner.is_file() {
        return Err(missing_or_broken(
            was_installed,
            format!("ALPHAFOLD3_RUNNER does not exist at {}.", runner.display()),
        ));
    }
    Ok(installer
        .tool_python_command(Tool::AlphaFold3)
        .arg(&runner)
        .arg("--help"))
}

/// For commands directly runnable from CLI, launch them with a "help" or "version" command as
/// a coarse status check.
fn probe_command(installer: &Installer, tool: Tool) -> Option<CommandSpec> {
    // The executable's name comes from `Tool::console_script`, which is not always the slug; only
    // the probe argument is decided here.
    let arguments: &[&str] = match tool {
        Tool::OpenDde => &["--version"],
        Tool::Boltz2
        | Tool::Chai1
        | Tool::Protenix
        | Tool::EsmFold2
        | Tool::ImmuneBuilder
        | Tool::BoltzGen
        | Tool::BioPhi
        | Tool::ProteinMpnnDdg
        | Tool::Anarcii
        | Tool::AggreScan3d
        | Tool::Mber => &["--help"],
        Tool::IgBlast => {
            let executable = installer.tools_root().join("igblast/bin/igblastn");
            return Some(CommandSpec::new(executable).arg("-version"));
        }
        Tool::Gromacs => {
            let prefix = installer
                .config
                .gromacs_prefix
                .clone()
                .unwrap_or_else(|| installer.tools_root().join("gromacs"));
            return Some(CommandSpec::new(prefix.join("bin/gmx")).arg("--version"));
        }
        Tool::BindCraft | Tool::Genie3 => {
            return named_conda_probe(installer, tool);
        }
        Tool::Germinal => {
            // The micromamba branch of the recipe builds a prefix environment; only the legacy
            // install.sh branch leaves a named Conda environment behind.
            let python = installer.venv_python(tool.slug());
            if python.is_file() {
                return Some(installer.tool_python_command(tool).arg("--version"));
            }
            return named_conda_probe(installer, tool);
        }
        _ => {
            return Some(installer.tool_python_command(tool).arg("--version"));
        }
    };
    Some(installer.tool_command(tool).args(arguments))
}

fn named_conda_probe(installer: &Installer, tool: Tool) -> Option<CommandSpec> {
    let executable = installer.conda_executable_path();
    let environment = tool.conda_environment()?;
    Some(CommandSpec::new(executable).args(["run", "--name", environment, "python", "--version"]))
}

fn run_probe(command: CommandSpec) -> Result<CommandOutput, RunError> {
    crate::run::run(
        &command
            .exit_policy(ExitPolicy::AllowFailure)
            .capture_limits(CaptureLimits::new(4_000, 4_000)),
    )
}

fn required_paths(installer: &Installer, tool: Tool) -> Vec<(PathBuf, &'static str)> {
    let root = installer.tools_root();
    let relative: &[(&str, &str)] = match tool {
        Tool::BindCraft => &[(
            "BindCraft/params/params_model_5_ptm.npz",
            "BindCraft weights",
        )],
        Tool::IgBlast => &[("igblast/internal_data", "IgBLAST internal data")],
        Tool::ProteinMpnn => &[
            ("ProteinMPNN/protein_mpnn_run.py", "ProteinMPNN runner"),
            (
                "ProteinMPNN/vanilla_model_weights/v_48_020.pt",
                "ProteinMPNN weights",
            ),
            ("ProteinMPNN/abmpnn_weights/v_48_020.pt", "ABMPNN weights"),
        ],
        Tool::LigandMpnn => &[
            ("LigandMPNN/run.py", "LigandMPNN runner"),
            (
                "LigandMPNN/model_params/ligandmpnn_v_32_010_25.pt",
                "LigandMPNN weights",
            ),
        ],
        Tool::RfDiffusion => &[
            ("RFdiffusion/scripts/run_inference.py", "RFdiffusion runner"),
            ("RFdiffusion/models/Base_ckpt.pt", "RFdiffusion weights"),
        ],
        Tool::RfAntibody => &[("RFantibody/weights/RFdiffusion_Ab.pt", "RFantibody weights")],
        Tool::Germinal => &[("germinal/run_germinal.py", "Germinal runner")],
        Tool::Mber => &[("mber-open", "mBER checkout")],
        Tool::IgDesign => &[("igdesign/predict.py", "IgDesign runner")],
        Tool::ThermoMpnn => &[(
            "ThermoMPNN/analysis/custom_inference.py",
            "ThermoMPNN runner",
        )],
        Tool::Genie3 => &[("genie3/pretrained", "Genie 3 weights")],
        Tool::DeepSp => &[("DeepSP/deepsp_cli.py", "DeepSP runner")],
        Tool::DeepImmuno => &[("DeepImmuno/deepimmuno-cnn.py", "DeepImmuno runner")],
        Tool::TlImmuno2 => &[("TLimmuno2/Python/TLimmuno2.py", "TLimmuno2 runner")],
        Tool::NetSolP => &[("NetSolP-1.0/PredictionServer", "NetSolP checkout")],
        Tool::DeepStabP => &[("deepStabP/src/Api/deepstabp_cli.py", "DeepSTABp runner")],
        Tool::DlkCat => &[(
            "DLKcat/DeeplearningApproach/Code/example/prediction_for_input.py",
            "DLKcat runner",
        )],
        Tool::CatPred => &[("CatPred/predict.py", "CatPred runner")],
        Tool::Placer => &[("PLACER/run_PLACER.py", "PLACER runner")],
        Tool::HighFold => &[("HighFold", "HighFold checkout")],
        Tool::AntiFold => &[("AntiFold", "AntiFold checkout")],
        _ => &[],
    };
    relative
        .iter()
        .map(|(path, description)| (root.join(path), *description))
        .collect()
}

fn probe_device(installer: &Installer, tool: Tool) -> Option<String> {
    let framework = if matches!(
        tool,
        Tool::HighFold | Tool::ProteinMpnnDdg | Tool::Germinal | Tool::Mber
    ) {
        "jax"
    } else if matches!(
        tool,
        Tool::OpenDde
            | Tool::Boltz2
            | Tool::Chai1
            | Tool::Protenix
            | Tool::EsmFold2
            | Tool::ImmuneBuilder
            | Tool::BoltzGen
            | Tool::ProteinMpnn
            | Tool::LigandMpnn
            | Tool::RfDiffusion
            | Tool::RfAntibody
            | Tool::IgDesign
            | Tool::ThermoMpnn
            | Tool::Genie3
            | Tool::DeepSp
            | Tool::NetSolP
            | Tool::DeepStabP
            | Tool::DlkCat
            | Tool::CatPred
            | Tool::Anarcii
            | Tool::Placer
    ) {
        "torch"
    } else {
        return None;
    };
    let python = installer.venv_python(tool.slug());
    if !python.is_file() {
        return None;
    }
    let snippet = if framework == "jax" {
        "import jax; print('GPU' if any(d.platform == 'gpu' for d in jax.devices()) else 'CPU')"
    } else {
        "import torch; print('GPU' if torch.cuda.is_available() else 'CPU')"
    };
    let command = installer
        .tool_python_command(tool)
        .args(["-c", snippet])
        .timeout(Duration::from_secs(300))
        .capture_limits(CaptureLimits::new(4_000, 4_000));
    let output = crate::run::run(&command).ok()?;
    if !output.status.success() {
        return None;
    }
    let value = output.stdout_lossy().trim().to_owned();
    matches!(value.as_str(), "GPU" | "CPU").then_some(value)
}

fn output_detail(output: &CommandOutput) -> String {
    let stdout = output.stdout_lossy();
    let stderr = output.stderr_lossy();
    format!("{stdout}{stderr}")
        .trim()
        .chars()
        .rev()
        .take(4_000)
        .collect::<String>()
        .chars()
        .rev()
        .collect()
}

fn pass(detail: impl Into<String>, device: Option<String>) -> ToolStatus {
    ToolStatus {
        result: StatusKind::Pass,
        detail: detail.into(),
        device,
    }
}

fn not_found(detail: impl Into<String>) -> ToolStatus {
    ToolStatus {
        result: StatusKind::NotFound,
        detail: detail.into(),
        device: None,
    }
}

fn error(detail: impl Into<String>) -> ToolStatus {
    ToolStatus {
        result: StatusKind::Error,
        detail: detail.into(),
        device: None,
    }
}

fn missing_or_broken(was_installed: bool, detail: String) -> ToolStatus {
    if was_installed {
        error(detail)
    } else {
        not_found(detail)
    }
}
