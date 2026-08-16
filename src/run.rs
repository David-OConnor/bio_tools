//! Reusable execution of third-party command-line tools.
//!
//! [`CommandSpec`] describes an invocation without tying it to a particular
//! biology tool. [`CommandRunner`] turns that description into a
//! [`std::process::Command`], captures bounded output, enforces an optional
//! timeout, and applies the selected ExitPolicy. Rust applications can
//! construct a [`CommandSpec`] directly or implement [`ToolInvocation`] for
//! their own tool-specific configuration type.

use std::{
    collections::BTreeMap,
    error::Error,
    ffi::{OsStr, OsString},
    fmt,
    fs::File,
    io::{self, Read, Write},
    path::PathBuf,
    process::{Command, ExitStatus, Stdio},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub use crate::run_log::RunLogSpec;
use crate::run_log::{ActiveRunLog, append_log_error};

/// Whether a non-zero exit status is returned to the caller or treated as an
/// execution error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExitPolicy {
    /// Return every completed process to the caller, irrespective of its exit
    /// status. This is useful for --help and --version probes.
    AllowFailure,
    /// Convert a non-zero exit status into RunError::ExitFailure.
    RequireSuccess,
}

/// Whether the child inherits the parent process's environment before command
/// specific values are applied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvironmentPolicy {
    Inherit,
    Clear,
}

/// Maximum bytes retained from the tail of each output stream.
///
/// Readers continue draining both pipes after a limit is reached. This avoids
/// deadlocking a verbose child while keeping application memory bounded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaptureLimits {
    pub stdout: usize,
    pub stderr: usize,
}

impl CaptureLimits {
    pub const fn new(stdout: usize, stderr: usize) -> Self {
        Self { stdout, stderr }
    }
}

impl Default for CaptureLimits {
    fn default() -> Self {
        Self::new(100_000, 100_000)
    }
}

/// A tool invocation independent of any particular application or tool
/// registry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandSpec {
    pub program: OsString,
    pub arguments: Vec<OsString>,
    pub current_dir: Option<PathBuf>,
    pub timeout: Option<Duration>,
    pub stdin: Option<Vec<u8>>,
    pub environment_policy: EnvironmentPolicy,
    pub environment: BTreeMap<OsString, OsString>,
    pub exit_policy: ExitPolicy,
    pub capture_limits: CaptureLimits,
    pub run_log: Option<RunLogSpec>,
}

impl CommandSpec {
    pub fn new(program: impl Into<OsString>) -> Self {
        Self {
            program: program.into(),
            arguments: Vec::new(),
            current_dir: None,
            timeout: Some(Duration::from_secs(120)),
            stdin: None,
            environment_policy: EnvironmentPolicy::Inherit,
            environment: BTreeMap::new(),
            exit_policy: ExitPolicy::RequireSuccess,
            capture_limits: CaptureLimits::default(),
            run_log: None,
        }
    }

    pub fn arg(mut self, argument: impl Into<OsString>) -> Self {
        self.arguments.push(argument.into());
        self
    }

    pub fn args<I, S>(mut self, arguments: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.arguments.extend(arguments.into_iter().map(Into::into));
        self
    }

    pub fn current_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.current_dir = Some(path.into());
        self
    }

    pub fn timeout(mut self, timeout: impl Into<Option<Duration>>) -> Self {
        self.timeout = timeout.into();
        self
    }

    pub fn stdin(mut self, input: impl Into<Vec<u8>>) -> Self {
        self.stdin = Some(input.into());
        self
    }

    pub fn environment_policy(mut self, policy: EnvironmentPolicy) -> Self {
        self.environment_policy = policy;
        self
    }

    pub fn env(mut self, name: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.environment.insert(name.into(), value.into());
        self
    }

    pub fn envs<I, K, V>(mut self, variables: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<OsString>,
        V: Into<OsString>,
    {
        self.environment.extend(
            variables
                .into_iter()
                .map(|(name, value)| (name.into(), value.into())),
        );
        self
    }

    pub fn exit_policy(mut self, policy: ExitPolicy) -> Self {
        self.exit_policy = policy;
        self
    }

    pub fn capture_limits(mut self, limits: CaptureLimits) -> Self {
        self.capture_limits = limits;
        self
    }

    pub fn run_log(mut self, settings: RunLogSpec) -> Self {
        self.run_log = Some(settings);
        self
    }

    /// A shell-free, diagnostic representation of the argument vector.
    pub fn display_command(&self) -> String {
        std::iter::once(self.program.as_os_str())
            .chain(self.arguments.iter().map(OsString::as_os_str))
            .map(display_argument)
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Implemented by tool-specific types that can describe their execution as a
/// generic CommandSpec.
pub trait ToolInvocation {
    fn command_spec(&self) -> CommandSpec;
}

impl ToolInvocation for CommandSpec {
    fn command_spec(&self) -> CommandSpec {
        self.clone()
    }
}

/// Captured output from a child process.
#[derive(Debug)]
pub struct CommandOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub elapsed: Duration,
    pub run_log_dir: Option<PathBuf>,
}

impl CommandOutput {
    pub fn return_code(&self) -> Option<i32> {
        self.status.code()
    }

    pub fn stdout_lossy(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }

    pub fn stderr_lossy(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }

    /// The most useful tail of failed-process output, preferring stderr.
    pub fn diagnostic(&self, maximum_characters: usize) -> String {
        let stderr = String::from_utf8_lossy(&self.stderr);
        let stdout = String::from_utf8_lossy(&self.stdout);
        let detail = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        tail_characters(detail, maximum_characters)
    }
}

/// Failures that occur before or while executing a tool command.
#[derive(Debug)]
pub enum RunError {
    EmptyProgram,
    Start {
        program: OsString,
        source: io::Error,
    },
    Input {
        source: io::Error,
    },
    Output {
        stream: &'static str,
        source: io::Error,
    },
    Wait {
        source: io::Error,
    },
    Log {
        action: &'static str,
        source: io::Error,
    },
    Timeout {
        timeout: Duration,
        output: CommandOutput,
    },
    ExitFailure {
        output: CommandOutput,
    },
}

impl RunError {
    pub fn output(&self) -> Option<&CommandOutput> {
        match self {
            Self::Timeout { output, .. } | Self::ExitFailure { output } => Some(output),
            _ => None,
        }
    }

    pub fn io_error_kind(&self) -> Option<io::ErrorKind> {
        match self {
            Self::Start { source, .. }
            | Self::Input { source }
            | Self::Output { source, .. }
            | Self::Wait { source }
            | Self::Log { source, .. } => Some(source.kind()),
            _ => None,
        }
    }
}

impl fmt::Display for RunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyProgram => formatter.write_str("No executable was provided."),
            Self::Start { source, .. } => {
                write!(formatter, "The tool could not be started: {source}.")
            }
            Self::Input { source } => {
                write!(
                    formatter,
                    "The tool's standard input could not be written: {source}."
                )
            }
            Self::Output { stream, source } => {
                write!(
                    formatter,
                    "The tool's {stream} could not be read: {source}."
                )
            }
            Self::Wait { source } => {
                write!(formatter, "The tool could not be waited on: {source}.")
            }
            Self::Log { action, source } => {
                write!(
                    formatter,
                    "The tool run could not be audited while {action}: {source}."
                )
            }
            Self::Timeout { timeout, .. } => write!(
                formatter,
                "The tool exceeded the {}-second request limit.",
                display_seconds(*timeout)
            ),
            Self::ExitFailure { output } => {
                let code = output
                    .return_code()
                    .map(|code| code.to_string())
                    .unwrap_or_else(|| "no exit code".to_owned());
                let detail = output.diagnostic(2_000);
                write!(
                    formatter,
                    "The tool exited with code {code}: {}",
                    if detail.is_empty() {
                        "No diagnostic output was produced."
                    } else {
                        &detail
                    }
                )
            }
        }
    }
}

impl Error for RunError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Start { source, .. }
            | Self::Input { source }
            | Self::Output { source, .. }
            | Self::Wait { source }
            | Self::Log { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Stateless command executor. The type gives applications a stable place to
/// add execution policy while keeping individual tool descriptions as data.
#[derive(Clone, Debug)]
pub struct CommandRunner {
    poll_interval: Duration,
}

impl Default for CommandRunner {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(10),
        }
    }
}

impl CommandRunner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn run_tool<T: ToolInvocation>(&self, tool: &T) -> Result<CommandOutput, RunError> {
        self.run(&tool.command_spec())
    }

    pub fn run(&self, spec: &CommandSpec) -> Result<CommandOutput, RunError> {
        if spec.program.is_empty() {
            return Err(RunError::EmptyProgram);
        }

        let run_log = spec
            .run_log
            .as_ref()
            .map(|settings| ActiveRunLog::start(spec, settings))
            .transpose()
            .map_err(|source| RunError::Log {
                action: "preparing the log",
                source,
            })?;

        let mut command = Command::new(&spec.program);
        command
            .args(&spec.arguments)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(current_dir) = &spec.current_dir {
            command.current_dir(current_dir);
        }

        if spec.environment_policy == EnvironmentPolicy::Clear {
            command.env_clear();
        }

        command.envs(&spec.environment);

        if spec.stdin.is_some() {
            command.stdin(Stdio::piped());
        } else {
            command.stdin(Stdio::null());
        }

        let stdout_log = run_log
            .as_ref()
            .map(ActiveRunLog::stdout_file)
            .transpose()
            .map_err(|source| RunError::Log {
                action: "opening stdout.txt",
                source,
            })?;
        let stderr_log = run_log
            .as_ref()
            .map(ActiveRunLog::stderr_file)
            .transpose()
            .map_err(|source| RunError::Log {
                action: "opening stderr.txt",
                source,
            })?;

        let started = Instant::now();
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(source) => {
                if let Some(run_log) = &run_log
                    && let Err(log_error) = run_log.record_start_error(&source)
                {
                    append_log_error(run_log.directory(), "recording start failure", &log_error);
                }
                return Err(RunError::Start {
                    program: spec.program.clone(),
                    source,
                });
            }
        };

        let stdout = child.stdout.take().expect("stdout was configured as piped");
        let stderr = child.stderr.take().expect("stderr was configured as piped");
        let stdout_limit = spec.capture_limits.stdout;
        let stderr_limit = spec.capture_limits.stderr;
        let stdout_reader = thread::spawn(move || read_tail(stdout, stdout_limit, stdout_log));
        let stderr_reader = thread::spawn(move || read_tail(stderr, stderr_limit, stderr_log));

        let input_writer = spec.stdin.as_ref().map(|input| {
            let mut stdin = child.stdin.take().expect("stdin was configured as piped");
            let input = input.clone();
            thread::spawn(move || match stdin.write_all(&input) {
                Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
                result => result,
            })
        });

        let deadline = spec
            .timeout
            .and_then(|timeout| started.checked_add(timeout));

        let (status, timed_out) = loop {
            match child.try_wait() {
                Ok(Some(status)) => break (status, false),
                Ok(None) => {}
                Err(source) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(RunError::Wait { source });
                }
            }

            if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                child.kill().map_err(|source| RunError::Wait { source })?;
                let status = child.wait().map_err(|source| RunError::Wait { source })?;
                break (status, true);
            }

            let sleep_for = deadline
                .map(|deadline| deadline.saturating_duration_since(Instant::now()))
                .map(|remaining| remaining.min(self.poll_interval))
                .unwrap_or(self.poll_interval);
            thread::sleep(sleep_for);
        };

        join_input(input_writer)?;
        let stdout = join_output(stdout_reader, "standard output")?;
        let stderr = join_output(stderr_reader, "standard error")?;

        let output = CommandOutput {
            status,
            stdout,
            stderr,
            elapsed: started.elapsed(),
            run_log_dir: run_log
                .as_ref()
                .map(|run_log| run_log.directory().to_owned()),
        };

        if let Some(run_log) = &run_log {
            run_log
                .finish(&output, timed_out)
                .map_err(|source| RunError::Log {
                    action: "finalizing the log",
                    source,
                })?;
        }

        if timed_out {
            return Err(RunError::Timeout {
                timeout: spec.timeout.unwrap_or_default(),
                output,
            });
        }
        if spec.exit_policy == ExitPolicy::RequireSuccess && !output.status.success() {
            return Err(RunError::ExitFailure { output });
        }
        Ok(output)
    }
}

/// Run one command with the default runner.
pub fn run(spec: &CommandSpec) -> Result<CommandOutput, RunError> {
    CommandRunner::default().run(spec)
}

fn read_tail(
    mut reader: impl Read,
    limit: usize,
    mut full_log: Option<File>,
) -> io::Result<Vec<u8>> {
    let mut retained = Vec::with_capacity(limit.min(8 * 1024));
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            if let Some(log) = &mut full_log {
                log.flush()?;
            }
            return Ok(retained);
        }
        if let Some(log) = &mut full_log {
            log.write_all(&buffer[..count])?;
        }
        if limit == 0 {
            continue;
        }
        let chunk = &buffer[..count];
        if chunk.len() >= limit {
            retained.clear();
            retained.extend_from_slice(&chunk[chunk.len() - limit..]);
            continue;
        }
        let overflow = retained
            .len()
            .saturating_add(chunk.len())
            .saturating_sub(limit);
        if overflow > 0 {
            retained.drain(..overflow);
        }
        retained.extend_from_slice(chunk);
    }
}

fn join_input(handle: Option<JoinHandle<io::Result<()>>>) -> Result<(), RunError> {
    let Some(handle) = handle else {
        return Ok(());
    };
    handle
        .join()
        .unwrap_or_else(|_| Err(io::Error::other("standard-input worker panicked")))
        .map_err(|source| RunError::Input { source })
}

fn join_output(
    handle: JoinHandle<io::Result<Vec<u8>>>,
    stream: &'static str,
) -> Result<Vec<u8>, RunError> {
    handle
        .join()
        .unwrap_or_else(|_| Err(io::Error::other("output worker panicked")))
        .map_err(|source| RunError::Output { stream, source })
}

fn tail_characters(value: &str, maximum: usize) -> String {
    value
        .chars()
        .rev()
        .take(maximum)
        .collect::<String>()
        .chars()
        .rev()
        .collect()
}

fn display_seconds(duration: Duration) -> String {
    let seconds = duration.as_secs_f64();
    if seconds.fract() == 0.0 {
        format!("{seconds:.0}")
    } else {
        format!("{seconds:.3}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_owned()
    }
}

fn display_argument(argument: &OsStr) -> String {
    let argument = argument.to_string_lossy();
    if argument.is_empty() || argument.chars().any(char::is_whitespace) {
        format!("{:?}", argument.as_ref())
    } else {
        argument.into_owned()
    }
}
