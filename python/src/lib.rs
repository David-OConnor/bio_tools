mod metadata;
mod run;

use std::{env, path::PathBuf};

use bio_tools_rs::{
    install::{Installer as RustInstaller, Tool, UninstallReport},
    run::{CaptureLimits, CommandSpec, ExitPolicy},
    status::{ToolStatus, StatusKind}
};
use pyo3::{exceptions::PyRuntimeError, prelude::*};

#[pyclass(name = "Status", frozen, skip_from_py_object)]
#[derive(Clone)]
struct PyStatus {
    #[pyo3(get)]
    result: String,
    #[pyo3(get)]
    detail: String,
    #[pyo3(get)]
    device: Option<String>,
}

impl From<ToolStatus> for PyStatus {
    fn from(status: ToolStatus) -> Self {
        let result = match status.result {
            StatusKind::Pass => "Pass",
            StatusKind::NotFound => "Can't find",
            StatusKind::Error => "Error",
        };
        Self {
            result: result.to_owned(),
            detail: status.detail,
            device: status.device,
        }
    }
}

#[pymethods]
impl PyStatus {
    fn __repr__(&self) -> String {
        format!(
            "Status(result={:?}, detail={:?}, device={:?})",
            self.result, self.detail, self.device
        )
    }
}

#[pyclass(name = "UninstallReport", frozen, skip_from_py_object)]
#[derive(Clone)]
struct PyUninstallReport {
    #[pyo3(get)]
    removed: Vec<String>,
    #[pyo3(get)]
    kept: Vec<String>,
}

impl From<UninstallReport> for PyUninstallReport {
    fn from(report: UninstallReport) -> Self {
        Self {
            removed: report
                .removed
                .into_iter()
                .map(|path| path.display().to_string())
                .collect(),
            kept: report.kept,
        }
    }
}

#[pymethods]
impl PyUninstallReport {
    fn __repr__(&self) -> String {
        format!(
            "UninstallReport(removed={:?}, kept={:?})",
            self.removed, self.kept
        )
    }
}

#[pyclass(name = "Tool", frozen, skip_from_py_object)]
#[derive(Clone)]
struct PyTool {
    slug: String,
    inner: Option<Tool>,
}

#[pymethods]
impl PyTool {
    #[new]
    fn new(slug: &str) -> PyResult<Self> {
        if let Ok(tool) = slug.parse::<Tool>() {
            return Ok(Self {
                slug: slug.to_owned(),
                inner: Some(tool),
            });
        }
        if matches!(slug, "rdkit" | "orca" | "pdbbind" | "tap") {
            return Ok(Self {
                slug: slug.to_owned(),
                inner: None,
            });
        }
        Err(pyo3::exceptions::PyValueError::new_err(format!(
            "unknown biology tool slug {slug:?}"
        )))
    }

    #[staticmethod]
    fn all() -> Vec<Self> {
        Tool::ALL
            .into_iter()
            .map(|tool| Self {
                slug: tool.slug().to_owned(),
                inner: Some(tool),
            })
            .collect()
    }

    #[getter]
    fn slug(&self) -> &str {
        &self.slug
    }

    #[getter]
    fn name(&self) -> String {
        self.inner
            .map(|tool| tool.name().to_owned())
            .unwrap_or_else(|| self.slug.clone())
    }

    /// Whether this slug has an automatic recipe on this operating system.
    #[getter]
    fn installable(&self) -> bool {
        self.inner.is_some_and(Tool::is_supported)
    }

    #[pyo3(signature = (process_executables, support_root=None))]
    fn install(
        &self,
        py: Python<'_>,
        process_executables: PathBuf,
        support_root: Option<PathBuf>,
    ) -> PyResult<()> {
        let tool = self.recipe()?;
        py.detach(move || configured_installer(process_executables, support_root)?.install(tool))
            .map_err(install_error)
    }

    #[pyo3(signature = (process_executables, support_root=None))]
    fn uninstall(
        &self,
        py: Python<'_>,
        process_executables: PathBuf,
        support_root: Option<PathBuf>,
    ) -> PyResult<PyUninstallReport> {
        let tool = self.recipe()?;
        py.detach(move || configured_installer(process_executables, support_root)?.uninstall(tool))
            .map(PyUninstallReport::from)
            .map_err(install_error)
    }

    #[pyo3(signature = (process_executables, support_root=None))]
    fn status(
        &self,
        py: Python<'_>,
        process_executables: PathBuf,
        support_root: Option<PathBuf>,
    ) -> PyResult<PyStatus> {
        if let Some(tool) = self.inner {
            return py
                .detach(move || {
                    let installer = configured_installer(process_executables, support_root)?;
                    Ok::<_, bio_tools_rs::install::InstallError>(installer.status(tool))
                })
                .map(PyStatus::from)
                .map_err(install_error);
        }
        let slug = self.slug.clone();
        let python_executable = py
            .import("sys")?
            .getattr("executable")?
            .extract::<PathBuf>()?;
        Ok(py.detach(move || external_status(&slug, &process_executables, &python_executable)))
    }

    fn __repr__(&self) -> String {
        format!("Tool({:?})", self.slug)
    }
}

impl PyTool {
    /// The recipe behind this slug, or the error for the slugs that have none.
    fn recipe(&self) -> PyResult<Tool> {
        self.inner.ok_or_else(|| {
            PyRuntimeError::new_err(format!(
                "{} is externally managed and has no automatic install recipe",
                self.slug
            ))
        })
    }
}

#[pyclass(name = "Installer", unsendable)]
struct PyInstaller {
    inner: RustInstaller,
}

#[pymethods]
impl PyInstaller {
    #[new]
    #[pyo3(signature = (process_executables, support_root=None))]
    fn new(process_executables: PathBuf, support_root: Option<PathBuf>) -> PyResult<Self> {
        Ok(Self {
            inner: configured_installer(process_executables, support_root)
                .map_err(install_error)?,
        })
    }

    fn install(&mut self, py: Python<'_>, tool: &PyTool) -> PyResult<()> {
        let tool = tool.recipe()?;
        py.detach(|| self.inner.install(tool))
            .map_err(install_error)
    }

    fn uninstall(&mut self, py: Python<'_>, tool: &PyTool) -> PyResult<PyUninstallReport> {
        let tool = tool.recipe()?;
        py.detach(|| self.inner.uninstall(tool))
            .map(PyUninstallReport::from)
            .map_err(install_error)
    }

    fn status(&self, py: Python<'_>, tool: &PyTool) -> PyResult<PyStatus> {
        let tool = tool.inner.ok_or_else(|| {
            PyRuntimeError::new_err(format!(
                "{} is externally managed; call Tool.status() for its external probe",
                tool.slug
            ))
        })?;
        Ok(PyStatus::from(py.detach(|| self.inner.status(tool))))
    }
}

fn configured_installer(
    process_executables: PathBuf,
    support_root: Option<PathBuf>,
) -> Result<RustInstaller, bio_tools_rs::install::InstallError> {
    let mut installer = RustInstaller::for_process_executables(process_executables)?;
    installer.config.support_root = support_root;
    Ok(installer)
}

fn install_error(error: bio_tools_rs::install::InstallError) -> PyErr {
    PyRuntimeError::new_err(error.to_string())
}

fn external_status(
    slug: &str,
    process_executables: &std::path::Path,
    python_executable: &std::path::Path,
) -> PyStatus {
    match slug {
        "rdkit" => command_status(CommandSpec::new(python_executable).args([
            "-c",
            "import rdkit; print('RDKit', rdkit.__version__)",
        ])),
        "orca" => {
            let configured = env::var_os("ORCA_EXECUTABLE").map(PathBuf::from);
            let executable = configured.filter(|path| path.is_file()).or_else(|| {
                [
                    process_executables.join("orca/orca"),
                    process_executables.join("ORCA/orca"),
                ]
                .into_iter()
                .find(|path| path.is_file())
            });
            let Some(executable) = executable else {
                return missing(
                    "ORCA is not configured under process_executables or ORCA_EXECUTABLE.",
                );
            };
            command_status(CommandSpec::new(executable).arg("--help"))
        }
        "pdbbind" => {
            let root = env::var_os("PDBBIND_ROOT")
                .map(PathBuf::from)
                .unwrap_or_else(|| process_executables.join("pdbbind"));
            if root.is_dir() {
                passing(format!("Dataset found at {}", root.display()), None)
            } else {
                missing(format!("No PDBBind release found at {}.", root.display()))
            }
        }
        "tap" => {
            let Some(python) = env::var_os("TAP_PYTHON").map(PathBuf::from) else {
                return missing("TAP_PYTHON is not configured.");
            };
            let Some(runner) = env::var_os("TAP_RUNNER").map(PathBuf::from) else {
                return missing("TAP_RUNNER is not configured.");
            };
            command_status(CommandSpec::new(python).arg(runner).arg("--help"))
        }
        _ => missing(format!("No status probe is defined for {slug}.")),
    }
}

fn command_status(command: CommandSpec) -> PyStatus {
    let command = command
        .exit_policy(ExitPolicy::AllowFailure)
        .capture_limits(CaptureLimits::new(4_000, 4_000));
    match bio_tools_rs::run::run(&command) {
        Ok(output) => {
            let detail = format!("{}{}", output.stdout_lossy(), output.stderr_lossy());
            let detail = detail.trim();
            if output.status.success() || (!detail.is_empty() && !detail.starts_with("Traceback")) {
                passing(
                    detail
                        .lines()
                        .next()
                        .unwrap_or("The tool answered its status probe."),
                    None,
                )
            } else {
                failing(if detail.is_empty() {
                    "The tool status probe exited unsuccessfully.".to_owned()
                } else {
                    detail.chars().take(200).collect()
                })
            }
        }
        Err(error) if error.io_error_kind() == Some(std::io::ErrorKind::NotFound) => {
            missing(error.to_string())
        }
        Err(error) => failing(error.to_string()),
    }
}

fn passing(detail: impl Into<String>, device: Option<String>) -> PyStatus {
    PyStatus {
        result: "Pass".to_owned(),
        detail: detail.into(),
        device,
    }
}

fn missing(detail: impl Into<String>) -> PyStatus {
    PyStatus {
        result: "Can't find".to_owned(),
        detail: detail.into(),
        device: None,
    }
}

fn failing(detail: impl Into<String>) -> PyStatus {
    PyStatus {
        result: "Error".to_owned(),
        detail: detail.into(),
        device: None,
    }
}

#[pymodule]
fn bio_tools(m: &Bound<'_, PyModule>) -> PyResult<()> {
    metadata::register(m)?;
    run::register(m)?;
    m.add_class::<PyTool>()?;
    m.add_class::<PyStatus>()?;
    m.add_class::<PyInstaller>()?;
    m.add_class::<PyUninstallReport>()?;
    Ok(())
}
