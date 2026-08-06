# Bio tools

An interface for running arbitrary CLI applications for biology and chemistry. Focuses on tools with permissive
licencing, and ones which are most popular. Handles installing these tools by downloading them, following official
installation procedures.

## Generic interfaces
Provides an interface for input and output. This abstracts over the differences between tools, so applications
can add many of them without repeating code


## Running tools

`run::CommandSpec` describes a shell-free invocation independently of any one
tool. `CommandRunner` builds a `std::process::Command`, overlays environment
variables, writes optional stdin, drains bounded stdout and stderr concurrently,
enforces a timeout, and either returns or rejects non-zero exits according to
`ExitPolicy`:

```rust,no_run
use std::time::Duration;
use bio_tools::run::{run, CommandSpec};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let command = CommandSpec::new("opendde")
        .args(["predict", "input.yaml"])
        .current_dir("work")
        .timeout(Duration::from_secs(600));
    let output = run(&command)?;
    println!("{}", output.stdout_lossy());
    Ok(())
}
```

## Installation
Handles installing applications. Details depend on the tool; some work by placing application executables in the
appropriate places. Since many of these use Python, it uses [uv](https://docs.astral.sh/uv/) to set up isolated environments 

The Rust installer replaces application-owned shell and PowerShell orchestration. The caller owns
the outer directory; `bio_tools` owns the stable per-tool layout, downloads, environments, GPU
selection, and verification:

```rust,no_run
use bio_tools::install::{Installer, Tool};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut installer = Installer::for_process_executables("./process_executables")?;
    installer.install(Tool::OpenDde)?;

    // Independent recipes continue after an upstream failure.
    let report = installer.install_many([Tool::Boltz2, Tool::ProteinMpnn]);
    if !report.is_success() {
        eprintln!("Failed installs: {:?}", report.failed);
    }
    Ok(())
}
```

`InstallLayout::process_executables` standardizes both consumers on assets under
`process_executables/` and environments under `process_executables/python_envs/`.
`InstallLayout::split` remains available for custom roots. A progress callback can be attached with
`Installer::with_reporter` for a GUI or structured setup log.

## Example uses
- Building a GUI (Web or native) to these tools
- Setting up an API to programmatically interface.


## Python bindings

The `python/` package builds an ABI3 wheel with PyO3 and maturin. It exposes
the same process metadata, command runner, installer, and status probes:

```python
from pathlib import Path
import bio_tools

root = Path("process_executables")
installer = bio_tools.Installer(root)
installer.install(bio_tools.Tool("opendde"))
print(bio_tools.Tool("opendde").status(root).result)

result = bio_tools.Command(
    ["opendde", "predict", "input.yaml"],
    cwd=Path("work"),
    timeout=600,
).run()
print(result.stdout)
```
