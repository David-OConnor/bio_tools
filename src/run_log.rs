use std::{
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    io::{self, BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::run::{CommandOutput, CommandSpec};

static RUN_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
pub(crate) fn test_sequence() -> u64 {
    RUN_SEQUENCE.fetch_add(1, Ordering::Relaxed)
}

/// Durable audit settings for one command execution.
///
/// Each execution gets a unique directory below `root/label`. The directory
/// records the invocation, complete output streams, optional stdin, and
/// before/after copies of the configured artifact paths.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunLogSpec {
    pub root: PathBuf,
    pub label: String,
    pub artifacts: Vec<PathBuf>,
}

impl RunLogSpec {
    pub fn new(root: impl Into<PathBuf>, label: impl Into<String>) -> Self {
        Self {
            root: root.into(),
            label: label.into(),
            artifacts: Vec::new(),
        }
    }

    pub fn artifact(mut self, path: impl Into<PathBuf>) -> Self {
        self.artifacts.push(path.into());
        self
    }

    pub fn artifacts<I, P>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.artifacts.extend(paths.into_iter().map(Into::into));
        self
    }
}

struct ArtifactCopy {
    source: PathBuf,
    input: PathBuf,
    output: PathBuf,
}

pub(crate) struct ActiveRunLog {
    directory: PathBuf,
    artifacts: Vec<ArtifactCopy>,
}

impl ActiveRunLog {
    pub(crate) fn start(spec: &CommandSpec, settings: &RunLogSpec) -> io::Result<Self> {
        let directory = unique_directory(&settings.root, &settings.label)?;
        fs::create_dir_all(directory.join("inputs"))?;
        fs::create_dir_all(directory.join("outputs"))?;

        let mut invocation = BufWriter::new(File::create(directory.join("invocation.txt"))?);
        writeln!(invocation, "command: {}", spec.display_command())?;
        writeln!(invocation, "argv:")?;
        writeln!(invocation, "  [0] {:?}", spec.program)?;
        for (index, argument) in spec.arguments.iter().enumerate() {
            writeln!(invocation, "  [{}] {:?}", index + 1, argument)?;
        }
        writeln!(
            invocation,
            "working directory: {}",
            spec.current_dir
                .as_deref()
                .map(Path::display)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "(inherited)".to_owned())
        )?;
        writeln!(
            invocation,
            "timeout seconds: {}",
            spec.timeout
                .map(|value| value.as_secs_f64().to_string())
                .unwrap_or_else(|| "none".to_owned())
        )?;
        writeln!(
            invocation,
            "environment policy: {:?}",
            spec.environment_policy
        )?;
        if spec.environment.is_empty() {
            writeln!(invocation, "environment overrides: none")?;
        } else {
            writeln!(invocation, "environment overrides:")?;
            for (name, value) in &spec.environment {
                writeln!(invocation, "  {:?}={:?}", name, value)?;
            }
        }
        invocation.flush()?;

        if let Some(stdin) = &spec.stdin {
            fs::write(directory.join("stdin.txt"), stdin)?;
        }

        let mut artifacts = Vec::new();
        let mut manifest = BufWriter::new(File::create(directory.join("artifacts.txt"))?);
        for (index, source) in settings.artifacts.iter().enumerate() {
            let source = absolute_artifact(source, spec.current_dir.as_deref())?;
            let name = source
                .file_name()
                .and_then(OsStr::to_str)
                .filter(|name| !name.is_empty())
                .unwrap_or("artifact");
            let entry = format!("{index:02}-{}", safe_component(name));
            let input = directory.join("inputs").join(&entry);
            let output = directory.join("outputs").join(&entry);
            writeln!(manifest, "{entry}: {}", source.display())?;
            if source.exists() || fs::symlink_metadata(&source).is_ok() {
                copy_path(&source, &input)?;
            } else {
                writeln!(manifest, "  absent before launch")?;
            }
            artifacts.push(ArtifactCopy {
                source,
                input,
                output,
            });
        }
        manifest.flush()?;

        Ok(Self {
            directory,
            artifacts,
        })
    }

    pub(crate) fn directory(&self) -> &Path {
        &self.directory
    }

    pub(crate) fn stdout_file(&self) -> io::Result<File> {
        File::create(self.directory.join("stdout.txt"))
    }

    pub(crate) fn stderr_file(&self) -> io::Result<File> {
        File::create(self.directory.join("stderr.txt"))
    }

    pub(crate) fn record_start_error(&self, error: &io::Error) -> io::Result<()> {
        fs::write(
            self.directory.join("error.txt"),
            format!("The command could not be started: {error}\n"),
        )?;
        self.write_combined_log(None, Some(&error.to_string()))
    }

    pub(crate) fn finish(&self, output: &CommandOutput, timed_out: bool) -> io::Result<()> {
        let mut deleted = Vec::new();
        for artifact in &self.artifacts {
            copy_changes(
                &artifact.source,
                &artifact.input,
                &artifact.output,
                Path::new(""),
                &mut deleted,
            )?;
        }
        if !deleted.is_empty() {
            fs::write(
                self.directory.join("deleted_files.txt"),
                deleted.join("\n") + "\n",
            )?;
        }

        let status = if timed_out {
            "timed out".to_owned()
        } else {
            output
                .return_code()
                .map(|code| format!("exit code {code}"))
                .unwrap_or_else(|| "terminated without an exit code".to_owned())
        };
        fs::write(
            self.directory.join("result.txt"),
            format!(
                "status: {status}\nelapsed seconds: {}\n",
                output.elapsed.as_secs_f64()
            ),
        )?;
        self.write_combined_log(Some(output), None)
    }

    fn write_combined_log(
        &self,
        output: Option<&CommandOutput>,
        start_error: Option<&str>,
    ) -> io::Result<()> {
        let mut combined = BufWriter::new(File::create(self.directory.join("run.log"))?);
        copy_section(
            &mut combined,
            "INVOCATION",
            &self.directory.join("invocation.txt"),
        )?;
        if self.directory.join("stdin.txt").is_file() {
            copy_section(&mut combined, "STDIN", &self.directory.join("stdin.txt"))?;
        } else {
            writeln!(
                combined,
                "\n===== STDIN =====\n(not supplied; the child received EOF)"
            )?;
        }
        if let Some(output) = output {
            writeln!(
                combined,
                "\n===== RESULT =====\nreturn code: {}\nelapsed seconds: {}",
                output
                    .return_code()
                    .map(|code| code.to_string())
                    .unwrap_or_else(|| "none".to_owned()),
                output.elapsed.as_secs_f64()
            )?;
        } else if let Some(error) = start_error {
            writeln!(combined, "\n===== RESULT =====\nstart error: {error}")?;
        }
        copy_section(&mut combined, "STDOUT", &self.directory.join("stdout.txt"))?;
        copy_section(&mut combined, "STDERR", &self.directory.join("stderr.txt"))?;
        combined.flush()
    }
}

fn unique_directory(root: &Path, label: &str) -> io::Result<PathBuf> {
    let tool_root = root.join(safe_component(label));
    fs::create_dir_all(&tool_root)?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    for _ in 0..100 {
        let sequence = RUN_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = tool_root.join(format!("{timestamp}-{}-{sequence}", std::process::id()));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a unique run-log directory",
    ))
}

fn safe_component(value: &str) -> String {
    let cleaned: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        "command".to_owned()
    } else {
        cleaned
    }
}

fn absolute_artifact(path: &Path, current_dir: Option<&Path>) -> io::Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_owned());
    }
    if let Some(current_dir) = current_dir {
        return Ok(current_dir.join(path));
    }
    Ok(std::env::current_dir()?.join(path))
}

fn copy_path(source: &Path, destination: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    if metadata.file_type().is_symlink() {
        return copy_symlink(source, destination);
    }
    if metadata.is_dir() {
        fs::create_dir_all(destination)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            copy_path(&entry.path(), &destination.join(entry.file_name()))?;
        }
        return Ok(());
    }
    if metadata.is_file() {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source, destination)?;
    }
    Ok(())
}

fn copy_changes(
    source: &Path,
    input: &Path,
    output: &Path,
    relative: &Path,
    deleted: &mut Vec<String>,
) -> io::Result<bool> {
    let source_metadata = fs::symlink_metadata(source).ok();
    let input_metadata = fs::symlink_metadata(input).ok();

    let Some(source_metadata) = source_metadata else {
        if input_metadata.is_some() {
            deleted.push(relative.to_string_lossy().into_owned());
        }
        return Ok(false);
    };

    if source_metadata.is_dir() {
        let mut changed = false;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let name = entry.file_name();
            changed |= copy_changes(
                &entry.path(),
                &input.join(&name),
                &output.join(&name),
                &relative.join(&name),
                deleted,
            )?;
        }
        if input_metadata.is_some_and(|metadata| metadata.is_dir()) {
            for entry in fs::read_dir(input)? {
                let entry = entry?;
                if fs::symlink_metadata(source.join(entry.file_name())).is_err() {
                    deleted.push(
                        relative
                            .join(entry.file_name())
                            .to_string_lossy()
                            .into_owned(),
                    );
                }
            }
        }
        return Ok(changed);
    }

    let unchanged = match input_metadata {
        Some(metadata)
            if metadata.file_type().is_symlink() && source_metadata.file_type().is_symlink() =>
        {
            fs::read_link(source)? == fs::read_link(input)?
        }
        Some(metadata) if metadata.is_file() && source_metadata.is_file() => {
            files_equal(source, input)?
        }
        _ => false,
    };
    if unchanged {
        return Ok(false);
    }
    copy_path(source, output)?;
    Ok(true)
}

fn files_equal(left: &Path, right: &Path) -> io::Result<bool> {
    if fs::metadata(left)?.len() != fs::metadata(right)?.len() {
        return Ok(false);
    }
    let mut left = BufReader::new(File::open(left)?);
    let mut right = BufReader::new(File::open(right)?);
    let mut left_buffer = [0_u8; 64 * 1024];
    let mut right_buffer = [0_u8; 64 * 1024];
    loop {
        let left_count = left.read(&mut left_buffer)?;
        let right_count = right.read(&mut right_buffer)?;
        if left_count != right_count || left_buffer[..left_count] != right_buffer[..right_count] {
            return Ok(false);
        }
        if left_count == 0 {
            return Ok(true);
        }
    }
}

fn copy_section(writer: &mut impl Write, name: &str, path: &Path) -> io::Result<()> {
    writeln!(writer, "\n===== {name} =====")?;
    if path.is_file() {
        io::copy(&mut File::open(path)?, writer)?;
    } else {
        writeln!(writer, "(not available)")?;
    }
    Ok(())
}

#[cfg(unix)]
fn copy_symlink(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::unix::fs::symlink;
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    symlink(fs::read_link(source)?, destination)
}

#[cfg(windows)]
fn copy_symlink(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::fs::{symlink_dir, symlink_file};
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    let target = fs::read_link(source)?;
    if source.is_dir() {
        symlink_dir(target, destination)
    } else {
        symlink_file(target, destination)
    }
}

pub(crate) fn append_log_error(directory: &Path, action: &str, error: &io::Error) {
    let _ = OpenOptions::new()
        .create(true)
        .append(true)
        .open(directory.join("logging_errors.txt"))
        .and_then(|mut file| writeln!(file, "{action}: {error}"));
}
