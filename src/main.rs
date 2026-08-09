//! Optional standalone CLI application; a thin wrapper around the library API which can be launched
//! without using it in a rust or python program

use std::{
    env,
    error::Error,
    ffi::OsString,
    path::{Path, PathBuf},
    process,
    str::FromStr,
};

use bio_tools::{
    install::{InstallConfig, InstallEvent, Installer, StatusKind, default_root},
    run::run,
    tool_definitions::{
        Tool,
        catalog::{self, CatalogEntry},
    },
};

fn format_status(status: &bio_tools::install::ToolStatus) -> String {
    match status.result {
        StatusKind::Pass => match &status.device {
            Some(device) => format!("Pass, {} {device}", status.detail),
            None => format!("Pass, {}", status.detail),
        },
        StatusKind::NotFound => "Not installed".to_owned(),
        StatusKind::Error => format!("Error: {}", status.detail),
    }
}

const USAGE: &str = "Usage:\n  bio_tools [--root <directory>] install <tool>\n  bio_tools [--root <directory>] uninstall <tool>\n  bio_tools [--root <directory>] status-quick <tool>\n  bio_tools [--root <directory>] status-full <tool>\n  bio_tools [--root <directory>] run <tool> [-- <tool arguments...>]\n  bio_tools [--root <directory>] list-quick\n  bio_tools [--root <directory>] list-full\n  bio_tools [--root <directory>] dir\n  bio_tools metadata <tool>\n\n`status` and `list` remain aliases for their full variants. `dir` prints the directory tools are installed to.\nThat directory is $BIO_TOOLS_ROOT when set, otherwise this platform's per-user data directory; `--root <directory>` overrides both.";

fn main() {
    if let Err(error) = real_main() {
        eprintln!("bio_tools: {error}");
        process::exit(1);
    }
}

fn real_main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args_os().skip(1).collect::<Vec<_>>();
    if args.is_empty() || matches!(args[0].to_str(), Some("-h" | "--help" | "help")) {
        println!("{USAGE}");
        return Ok(());
    }
    let (root, root_source) = take_root(&mut args)?;
    let command = args
        .first()
        .and_then(|value| value.to_str())
        .ok_or("missing command")?
        .to_owned();
    args.remove(0);
    if command == "dir" {
        if !args.is_empty() {
            return Err("dir does not accept arguments".into());
        }
        print_dir(&root, &root_source);
        return Ok(());
    }
    if matches!(command.as_str(), "list" | "list-full" | "list-quick") {
        if !args.is_empty() {
            return Err(format!("{command} does not accept arguments").into());
        }
        let installer = Installer::from_environment(root)?;

        println!("\nStatuses by tool\n====================");
        for tool in Tool::ALL {
            let status = if command == "list-quick" {
                installer.status_quick(tool)
            } else {
                installer.status_full(tool)
            };
            println!("- {}: {}", tool.name(), format_status(&status));
        }
        println!("====================");

        return Ok(());
    }
    if command == "metadata" {
        let slug = args
            .first()
            .and_then(|value| value.to_str())
            .ok_or("missing tool name")?
            .to_owned();
        args.remove(0);
        require_empty(&args, "metadata")?;
        let entry = catalog::by_slug(&slug).ok_or_else(|| {
            let slugs = catalog::ALL
                .iter()
                .map(|entry| entry.slug())
                .collect::<Vec<_>>()
                .join(", ");
            format!("unknown tool {slug:?}; known tools: {slugs}")
        })?;
        print_metadata(entry);
        return Ok(());
    }
    let tool = Tool::from_str(
        args.first()
            .and_then(|value| value.to_str())
            .ok_or("missing tool name")?,
    )?;
    args.remove(0);
    let mut installer = Installer::from_environment(root)?.with_reporter(progress);
    match command.as_str() {
        "install" => {
            require_empty(&args, "install")?;
            installer.install(tool)?;
        }
        "uninstall" => {
            require_empty(&args, "uninstall")?;
            let report = installer.uninstall(tool)?;
            if report.removed.is_empty() {
                println!("{} was already absent.", tool.name());
            }
            for path in report.removed {
                println!("Removed {}", path.display());
            }
            for note in report.kept {
                println!("Kept: {note}");
            }
        }
        "status" | "status-full" | "status_full" | "status-quick" | "status_quick" => {
            require_empty(&args, &command)?;
            let status = if matches!(command.as_str(), "status-quick" | "status_quick") {
                installer.status_quick(tool)
            } else {
                installer.status_full(tool)
            };
            println!("{}: {:?}\n{}", tool.name(), status.result, status.detail);
            if let Some(device) = status.device {
                println!("Device: {device}");
            }
            if status.result != StatusKind::Pass {
                process::exit(2);
            }
        }
        "run" => run_tool(&installer, tool, args)?,
        _ => return Err(format!("unknown command {command:?}\n\n{USAGE}").into()),
    }
    Ok(())
}

fn print_metadata(entry: &CatalogEntry) {
    let categories = entry
        .categories
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    println!("{} ({})\n{}", entry.name(), entry.slug(), "=".repeat(56));
    println!("Categories: {categories}");
    println!("Launch type: {}", entry.launch_type);
    println!("License type: {}", entry.license_type);
    println!("Expense: {}", entry.expense);
    println!("Top choice: {}", entry.top_choice);
    println!(
        "Install recipe: {}",
        match entry.identity.tool() {
            Some(tool) => tool.slug(),
            None => "none (not installable by bio_tools)",
        }
    );

    let spec = &entry.spec;
    println!("\nSpec\n{}", "-".repeat(56));
    println!("Summary: {}", spec.summary);
    println!("Description: {}", spec.description);
    println!("Availability: {}", spec.availability);
    println!("License: {}", spec.license);
    println!("License details: {}", spec.license_details);
    for (label, url) in [
        (
            "License URL",
            spec.license.official_url().or(spec.license_url),
        ),
        ("Repo URL", spec.repo_url),
        ("Home URL", spec.home_url),
        ("Docs URL", spec.docs_url),
        ("Paper URL", spec.paper_url),
    ] {
        if let Some(url) = url {
            println!("{label}: {url}");
        }
    }
}

/// Where the installation root came from, so `dir` can say which knob to reach for.
enum RootSource {
    Flag,
    Environment,
    Platform,
}

impl RootSource {
    fn describe(&self) -> &'static str {
        match self {
            Self::Flag => "--root",
            Self::Environment => "$BIO_TOOLS_ROOT",
            Self::Platform => "this platform's per-user data directory (the default)",
        }
    }
}

fn take_root(args: &mut Vec<OsString>) -> Result<(PathBuf, RootSource), Box<dyn Error>> {
    if args.first().is_some_and(|arg| arg == "--root") {
        if args.len() < 2 {
            return Err("--root needs a directory".into());
        }
        args.remove(0);
        return Ok((PathBuf::from(args.remove(0)), RootSource::Flag));
    }
    // A root relative to the working directory would put a separate copy of every environment and
    // model file beside each directory bio_tools happens to be launched from, so the fallback is
    // the platform's canonical per-user location rather than `./.bio_tools`.
    match env::var_os("BIO_TOOLS_ROOT").filter(|value| !value.is_empty()) {
        Some(value) => Ok((PathBuf::from(value), RootSource::Environment)),
        None => Ok((default_root(), RootSource::Platform)),
    }
}

/// Report the resolved installation root, without creating or touching it.
fn print_dir(root: &Path, source: &RootSource) {
    let layout = &InstallConfig::new(root).layout;
    println!(
        "Installation root: {}{}",
        root.display(),
        if root.is_dir() {
            ""
        } else {
            " (does not exist yet)"
        }
    );
    println!("  Tool assets:         {}", layout.tools_root.display());
    println!(
        "  Python environments: {}",
        layout.environment("<tool>").display()
    );
    println!("Set by: {}", source.describe());
}

fn run_tool(
    installer: &Installer,
    tool: Tool,
    mut args: Vec<OsString>,
) -> Result<(), Box<dyn Error>> {
    if args.first().is_some_and(|arg| arg == "--") {
        args.remove(0);
    }
    let executable = installer.executable_path(tool);
    if !executable.is_file() {
        return Err(format!("{} has no installed console entry point at {}; install it first, or use the library API for a tool-specific invocation", tool.name(), executable.display()).into());
    }
    let output = run(&installer.tool_command(tool).args(args).timeout(None))?;
    print!("{}", output.stdout_lossy());
    eprint!("{}", output.stderr_lossy());
    Ok(())
}

fn progress(event: InstallEvent) {
    match event {
        InstallEvent::ToolStarted(tool) => println!("\n{tool}\n{}", "=".repeat(56)),
        InstallEvent::Step { description, .. }
        | InstallEvent::Note {
            message: description,
            ..
        } => println!("  {description}"),
        InstallEvent::ToolFinished(_) => {}
    }
}
fn require_empty(args: &[OsString], command: &str) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        Ok(())
    } else {
        Err(format!("{command} accepts only one tool name").into())
    }
}
