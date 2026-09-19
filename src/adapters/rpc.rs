//! File-based bridge for Python adapters hosted by a Rust application.
//!
//! The adapter coordinator does not need a separately published Python wheel:
//! catalog queries and model execution go through the library linked by its host.
use std::{fs, io, path::Path, time::Duration};

use serde_json::{Value, json};
use crate::{run::{self, CommandSpec, CaptureLimits, ExitPolicy, RunLogSpec}, tool_definitions::{fields, presets}};

/// Handle one request without starting the host application's GUI.
pub fn serve(request: &Path, response: &Path) -> io::Result<()> {
    let result = fs::read(request)
        .map_err(|error| error.to_string())
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).map_err(|error| error.to_string()))
        .and_then(dispatch);
    let envelope = match result {
        Ok(value) => json!({"result": value}),
        Err(error) => json!({"error": error}),
    };
    fs::write(response, serde_json::to_vec(&envelope)?)
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value.get(key).and_then(Value::as_str).ok_or_else(|| format!("Missing {key}"))
}

fn dispatch(request: Value) -> Result<Value, String> {
    let op = text(&request, "op")?;
    match op {
        "catalog" => {
            let slug = text(&request, "slug")?;
            let document = fields::by_slug(slug).ok_or_else(|| format!("Unknown tool {slug}"))?;
            serde_json::from_str(document).map_err(|error| error.to_string())
        }
        "preset" => {
            let overrides = request.get("overrides").and_then(Value::as_object).cloned().unwrap_or_default();
            let slug = text(&request, "slug")?;
            let payload = presets::payload(slug, text(&request, "preset")?, &overrides).map_err(|error| error.to_string())?;
            if let Some(path) = request.get("workdir").and_then(Value::as_str) {
                presets::materialize(slug, &payload, Path::new(path)).map_err(|error| error.to_string())
            } else { Ok(payload) }
        }
        "input_text" => presets::input_text(text(&request, "slug")?, text(&request, "value")?)
            .map(|value| json!(value)).map_err(|error| error.to_string()),
        "asset" => presets::asset(text(&request, "slug")?, text(&request, "name")?)
            .map(|value| json!(value)).ok_or_else(|| "Unknown bundled asset".to_owned()),
        "chai1_example_msas" => {
            let installer = crate::install::Installer::for_process_executables(text(&request, "root")?)
                .map_err(|error| error.to_string())?;
            installer.ensure_chai1_example_msas().map(|path| json!(path)).map_err(|error| error.to_string())
        }
        "run" => execute(&request),
        _ => Err(format!("Unknown adapter operation: {op}")),
    }
}

fn execute(request: &Value) -> Result<Value, String> {
    let arguments = request.get("command").and_then(Value::as_array)
        .ok_or_else(|| "Missing command".to_owned())?
        .iter().map(|value| value.as_str().ok_or_else(|| "Command arguments must be strings".to_owned()))
        .collect::<Result<Vec<_>, _>>()?;
    let (program, arguments) = arguments.split_first().ok_or_else(|| "Empty command".to_owned())?;
    let mut command = CommandSpec::new(program).args(arguments.iter().copied());
    if let Some(cwd) = request.get("cwd").and_then(Value::as_str) { command = command.current_dir(cwd); }
    if let Some(timeout) = request.get("timeout").and_then(Value::as_f64) {
        if !timeout.is_finite() || timeout <= 0.0 { return Err("Timeout must be positive".to_owned()); }
        command = command.timeout(Duration::from_secs_f64(timeout));
    }
    if let Some(stdin) = request.get("stdin").and_then(Value::as_str) { command = command.stdin(stdin.as_bytes()); }
    if let Some(environment) = request.get("env").and_then(Value::as_object) {
        for (key, value) in environment {
            command = command.env(key, value.as_str().ok_or_else(|| "Environment values must be strings".to_owned())?);
        }
    }
    if request.get("check").and_then(Value::as_bool) == Some(false) {
        command = command.exit_policy(ExitPolicy::AllowFailure);
    }
    let limit = request.get("output_limit").and_then(Value::as_u64).unwrap_or(100_000) as usize;
    command = command.capture_limits(CaptureLimits::new(limit, limit));
    if let Some(root) = request.get("run_log_dir").and_then(Value::as_str) {
        let mut log = RunLogSpec::new(root, request.get("run_name").and_then(Value::as_str).unwrap_or("adapter"));
        if let Some(artifacts) = request.get("artifacts").and_then(Value::as_array) {
            for artifact in artifacts {
                log = log.artifact(artifact.as_str().ok_or_else(|| "Artifact paths must be strings".to_owned())?);
            }
        }
        command = command.run_log(log);
    }
    let display = command.display_command();
    let result = run::run(&command).map_err(|error| error.to_string())?;
    Ok(json!({
        "command": display, "return_code": result.return_code(),
        "stdout": result.stdout_lossy(), "stderr": result.stderr_lossy(),
        "run_log_dir": result.run_log_dir,
    }))
}
