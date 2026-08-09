use bio_tools::{install::Installer, tool_definitions::Tool};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut installer = Installer::for_process_executables("process_executables")?;
    installer.install(Tool::OpenDde)?;

    // Independent recipes continue after an upstream failure.
    let report = installer.install_many([Tool::Boltz2, Tool::ProteinMpnn]);
    for failure in &report.failed {
        eprintln!("{}: {}", failure.tool.name(), failure.error);
    }

    // Status: `status_quick` inspects markers, executables, and required assets
    // without launching the tool; `status_full` also runs its help/version probe.
    let status = installer.status_quick(Tool::OpenDde);
    println!("{:?}: {}", status.result, status.detail);

    let report = installer.uninstall(Tool::OpenDde)?;
    println!("Removed {} paths", report.removed.len());
    Ok(())
}
