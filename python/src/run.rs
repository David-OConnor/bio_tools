use std::{collections::HashMap, path::PathBuf, time::Duration};

use bio_tools_rs::run::{CaptureLimits, CommandSpec, ExitPolicy, RunLogSpec};
use pyo3::{create_exception, exceptions::PyRuntimeError, prelude::*};

create_exception!(bio_tools, CommandError, PyRuntimeError);

#[pyclass(
    name = "CommandOutput",
    module = "bio_tools",
    frozen,
    skip_from_py_object
)]
pub(crate) struct PyCommandOutput {
    #[pyo3(get)]
    command: Vec<String>,
    #[pyo3(get)]
    return_code: Option<i32>,
    #[pyo3(get)]
    stdout: String,
    #[pyo3(get)]
    stderr: String,
    #[pyo3(get)]
    elapsed_seconds: f64,
    #[pyo3(get)]
    run_log_dir: Option<PathBuf>,
}

/// Execute a prepared Rust command specification for any Python-facing API.
pub(crate) fn execute(
    py: Python<'_>,
    spec: CommandSpec,
    command: Vec<String>,
) -> PyResult<PyCommandOutput> {
    py.detach(move || bio_tools_rs::run::run(&spec))
        .map(|output| PyCommandOutput {
            command,
            return_code: output.return_code(),
            stdout: output.stdout_lossy(),
            stderr: output.stderr_lossy(),
            elapsed_seconds: output.elapsed.as_secs_f64(),
            run_log_dir: output.run_log_dir,
        })
        .map_err(|error| CommandError::new_err(error.to_string()))
}

#[pymethods]
impl PyCommandOutput {
    fn __repr__(&self) -> String {
        format!(
            "CommandOutput(command={:?}, return_code={:?}, stdout={:?}, stderr={:?})",
            self.command, self.return_code, self.stdout, self.stderr
        )
    }
}

/// A shell-free invocation executed by bio_tools' Rust command runner.
#[pyclass(name = "Command", module = "bio_tools", frozen, skip_from_py_object)]
pub(crate) struct PyCommand {
    command: Vec<String>,
    cwd: Option<PathBuf>,
    timeout: Duration,
    stdin: Option<String>,
    env: HashMap<String, String>,
    check: bool,
    output_limit: usize,
    run_log_dir: Option<PathBuf>,
    run_name: Option<String>,
    artifacts: Option<Vec<PathBuf>>,
}

#[pymethods]
impl PyCommand {
    #[new]
    #[pyo3(signature = (
        command,
        *,
        cwd=None,
        timeout=120.0,
        stdin=None,
        env=None,
        check=true,
        output_limit=100_000,
        run_log_dir=None,
        run_name=None,
        artifacts=None
    ))]
    fn new(
        command: Vec<String>,
        cwd: Option<PathBuf>,
        timeout: f64,
        stdin: Option<String>,
        env: Option<HashMap<String, String>>,
        check: bool,
        output_limit: usize,
        run_log_dir: Option<PathBuf>,
        run_name: Option<String>,
        artifacts: Option<Vec<PathBuf>>,
    ) -> PyResult<Self> {
        if command.first().is_none_or(String::is_empty) {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "command must begin with an executable",
            ));
        }
        if !timeout.is_finite() || timeout < 0.0 {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "timeout must be a finite, non-negative number of seconds",
            ));
        }
        Ok(Self {
            command,
            cwd,
            timeout: Duration::from_secs_f64(timeout),
            stdin,
            env: env.unwrap_or_default(),
            check,
            output_limit,
            run_log_dir,
            run_name,
            artifacts,
        })
    }

    fn run(&self, py: Python<'_>) -> PyResult<PyCommandOutput> {
        let mut spec = CommandSpec::new(&self.command[0])
            .args(self.command[1..].iter().cloned())
            .timeout(self.timeout)
            .envs(self.env.clone())
            .exit_policy(if self.check {
                ExitPolicy::RequireSuccess
            } else {
                ExitPolicy::AllowFailure
            })
            .capture_limits(CaptureLimits::new(self.output_limit, self.output_limit));
        if let Some(cwd) = &self.cwd {
            spec = spec.current_dir(cwd);
        }
        if let Some(stdin) = &self.stdin {
            spec = spec.stdin(stdin.as_bytes().to_vec());
        }
        if let Some(root) = &self.run_log_dir {
            let label = self
                .run_name
                .clone()
                .unwrap_or_else(|| command_label(&self.command[0]));
            let artifacts = self
                .artifacts
                .clone()
                .unwrap_or_else(|| self.cwd.iter().map(|_| PathBuf::from(".")).collect());
            spec = spec.run_log(RunLogSpec::new(root, label).artifacts(artifacts));
        }

        execute(py, spec, self.command.clone())
    }

    fn __repr__(&self) -> String {
        format!(
            "Command({:?}, cwd={:?}, timeout={:?}, check={})",
            self.command, self.cwd, self.timeout, self.check
        )
    }
}

pub(crate) fn command_label(program: &str) -> String {
    std::path::Path::new(program)
        .file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("command")
        .to_owned()
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add("CommandError", module.py().get_type::<CommandError>())?;
    module.add_class::<PyCommand>()?;
    module.add_class::<PyCommandOutput>()?;
    Ok(())
}
