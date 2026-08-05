use std::{env, path::PathBuf, process::Command};

use bio_tools_rs::{
    install::{Installer as RustInstaller, Tool},
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

    fn install(
        &self,
        py: Python<'_>,
        process_executables: PathBuf,
        support_root: Option<PathBuf>,
    ) -> PyResult<()> {
        let tool = self.inner.ok_or_else(|| {
            PyRuntimeError::new_err(format!(
                "{} is externally managed and has no automatic install recipe",
                self.slug
            ))
        })?;
        py.detach(move || configured_installer(process_executables, support_root)?.install(tool))
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
        let tool = tool.inner.ok_or_else(|| {
            PyRuntimeError::new_err(format!(
                "{} is externally managed and has no automatic install recipe",
                tool.slug
            ))
        })?;
        py.detach(|| self.inner.install(tool))
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
        "rdkit" => command_status(
            Command::new(python_executable)
                .args(["-c", "import rdkit; print('RDKit', rdkit.__version__)"]),
        ),
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
            command_status(Command::new(executable).arg("--help"))
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
            command_status(Command::new(python).arg(runner).arg("--help"))
        }
        _ => missing(format!("No status probe is defined for {slug}.")),
    }
}

fn command_status(command: &mut Command) -> PyStatus {
    match command.output() {
        Ok(output) => {
            let detail = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
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
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => missing(error.to_string()),
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
    m.add_class::<PyTool>()?;
    m.add_class::<PyStatus>()?;
    m.add_class::<PyInstaller>()?;
    Ok(())
}
