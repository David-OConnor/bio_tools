//! Optional standalone CLI application; a thin wrapper around the library API which can be launched
//! without using it in a rust or python program

use std::{env, error::Error, ffi::OsString, path::PathBuf, process, str::FromStr};

use bio_tools::{
    install::{InstallEvent, Installer, StatusKind},
    run::run,
    tool_definitions::Tool,
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

const USAGE: &str = "Usage:\n  bio_tools [--root <directory>] install <tool>\n  bio_tools [--root <directory>] uninstall <tool>\n  bio_tools [--root <directory>] status-quick <tool>\n  bio_tools [--root <directory>] status-full <tool>\n  bio_tools [--root <directory>] run <tool> [-- <tool arguments...>]\n  bio_tools [--root <directory>] list-quick\n  bio_tools [--root <directory>] list-full\n\n`status` and `list` remain aliases for their full variants. The installation root defaults to $BIO_TOOLS_ROOT, or ./.bio_tools when unset.";

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
    let root = take_root(&mut args)?;
    let command = args
        .first()
        .and_then(|value| value.to_str())
        .ok_or("missing command")?
        .to_owned();
    args.remove(0);
    if matches!(command.as_str(), "list" | "list-full" | "list-quick") {
        if !args.is_empty() {
            return Err(format!("{command} does not accept arguments").into());
        }
        let installer = Installer::from_environment(root)?;
        for tool in Tool::ALL {
            let status = if command == "list-quick" {
                installer.status_quick(tool)
            } else {
                installer.status_full(tool)
            };
            println!("{}: {}", tool.name(), format_status(&status));
        }
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

fn take_root(args: &mut Vec<OsString>) -> Result<PathBuf, Box<dyn Error>> {
    if args.first().is_some_and(|arg| arg == "--root") {
        if args.len() < 2 {
            return Err("--root needs a directory".into());
        }
        args.remove(0);
        return Ok(PathBuf::from(args.remove(0)));
    }
    Ok(env::var_os("BIO_TOOLS_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".bio_tools")))
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
