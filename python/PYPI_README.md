# Bio tools

[![Crate](https://img.shields.io/crates/v/bio_tools.svg)](https://crates.io/crates/bio_tools)
[![Docs](https://docs.rs/bio_tools/badge.svg)](https://docs.rs/bio_tools)
[![PyPI](https://img.shields.io/pypi/v/athanor-bio-tools.svg)](https://pypi.org/project/athanor-bio-tools)

[Home page](https://www.athanorlab.com/rust-tools)

An interface for running arbitrary CLI applications for biology and chemistry. It focuses on tools with permissive
licencing, and ones which are most popular. Available as a rust library, a python library, and a standalone
CLI application.

Includes the most popular tools for structure prediction, sequence prediction, and drug design broadly. For example:

- [AlphaFold 3](https://deepmind.google/science/alphafold/)
- [ProteinMPNN](https://github.com/dauparas/ProteinMPNN) and [LigandMPNN](https://github.com/dauparas/LigandMPNN)
- [Boltz-2](https://boltz.bio/) / [BoltzGen](https://boltz.bio/boltzgen)
- [RFdiffusion](https://sites.google.com/omsf.io/rfdiffusion) and [RFantibody](https://github.com/RosettaCommons/RFantibody)
- [Chai-1](https://www.chaidiscovery.com/)
- [Protenix](https://protenix-server.com/)
- [BindCraft](https://github.com/martinpacesa/BindCraft)
- [OpenDDE](https://aurekaresearch.github.io/OpenDDE-Website/)
- [ImmuneBuilder](https://opig.stats.ox.ac.uk/webapps/sabdab-sabpred/sabpred/abodybuilder2/)
- [ThermoMPNN](https://github.com/Kuhlman-Lab/ThermoMPNN)

Around 35 more are covered; see `Tool::ALL` and `tool_definitions::catalog` for the full set, each with its
own summary, license, and official links.

Handles the following tasks:
- Install
- Uninstall
- Run (Including abstractions over what inputs are accepted per tool)
- Check status


## Quickstart

### As a standalone CLI application

Install a prebuilt binary for Linux, Windows, or Mac from the
[Releases page](https://github.com/David-OConnor/bio_tools/releases), or build it with Cargo:

```sh
cargo install bio_tools
```

Either way you end up with `bio_tools` on your path.

### As a Rust library

```sh
cargo add bio_tools
```

### As a Python library

The PyPI distribution is named `athanor_bio_tools`. The module you import is `bio_tools`.

```sh
pip install athanor_bio_tools
```

```sh
uv add athanor_bio_tools
```

### Usage
Run the program with no parameters to see its functionality:
```bash
Usage:
  bio_tools [--root <directory>] install <tool>
  bio_tools [--root <directory>] uninstall <tool>
  bio_tools [--root <directory>] status-quick <tool>
  bio_tools [--root <directory>] status-full <tool>
  bio_tools [--root <directory>] run <tool> [-- <tool arguments...>]
  bio_tools [--root <directory>] list-quick
  bio_tools [--root <directory>] list-full
  bio_tools metadata <tool>
```

Examples:
- `bio_tools install boltz`
- `bio_tools uninstall proteinmpnn`
- `bio_tools list-quick`
- 

## Generic interfaces and code consolidation

This library provides an interface for input and output. This abstracts over the differences between tools, so applications
can add many of them without repeating code. This library was built as the backbone of the
[Athanor Bio Tools](https://athanortools.com/) web UI, and the external tool integrations in [Molchanica](https://www.athanorlab.com/molchanica). These use the Python and Rust libraries respectively. *Bio Tools* is designed
to reduce repetition between these projects.

The CLI application is intended for cases where you're not writing software, but want to easily
install these tools directly, without handling the system dependencies and python environments
for each tool.


## Installing tools

Handles installing applications. Details depend on the tool; some work by placing application executables in the
appropriate places. Since many of these use Python, it uses [uv](https://docs.astral.sh/uv/) to set up isolated environments.

The Rust installer replaces application-owned shell and PowerShell orchestration. The caller owns
the outer directory; `bio_tools` owns the stable per-tool layout, downloads, environments, GPU
selection, and verification.

`InstallLayout::process_executables` standardizes both consumers on assets under
`process_executables/` and environments under `process_executables/python_envs/`.
`InstallLayout::split` remains available for custom roots. A progress callback can be attached with
`Installer::with_reporter` for a GUI or structured setup log.

**Rust:**

```rust
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
```

**Python** (equivalent):

```python
from pathlib import Path
import bio_tools

root = Path("process_executables")
installer = bio_tools.Installer(root)
installer.install(bio_tools.Tool("opendde"))

# Independent recipes continue after an upstream failure.
for slug in ("boltz2", "proteinmpnn"):
    try:
        installer.install(bio_tools.Tool(slug))
    except RuntimeError as error:
        print(f"{slug}: {error}")

status = installer.status_quick(bio_tools.Tool("opendde"))
print(status.result, status.detail)

report = installer.uninstall(bio_tools.Tool("opendde"))
print(f"Removed {len(report.removed)} paths")
```


## Running tools

`run::CommandSpec` describes a shell-free invocation independently of any one
tool. `CommandRunner` builds a `std::process::Command`, overlays environment
variables, writes optional stdin (or closes it when absent), drains bounded
stdout and stderr concurrently, enforces a timeout, and either returns or
rejects non-zero exits according to `ExitPolicy`.

**Rust:**

```rust
use std::time::Duration;

use bio_tools::run::{CommandSpec, RunLogSpec, run};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let command = CommandSpec::new("opendde")
        .args(["predict", "input.yaml"])
        .current_dir("work")
        .timeout(Duration::from_secs(600))
        .run_log(RunLogSpec::new("process_executables/run_logs", "opendde").artifact("."));

    let output = run(&command)?;
    println!("{}", output.stdout_lossy());
    Ok(())
}
```

**Python** (equivalent):

```python
from pathlib import Path
import bio_tools

result = bio_tools.Command(
    ["opendde", "predict", "input.yaml"],
    cwd=Path("work"),
    timeout=600,
    run_log_dir=Path("process_executables/run_logs"),
    run_name="opendde",
).run()

print(result.stdout)
print(result.run_log_dir)
```

`Installer::tool_command` (Python: `Installer.run`) is the variant to reach for when the tool lives in a
managed environment rather than on `PATH`; it resolves the installed console entry point for you.

### Run logs

When a run log is configured, each invocation gets a unique directory below the given root and run
name. `run.log` combines the exact argument vector, optional stdin, result, and complete
stdout/stderr. The same streams are also available as `stdout.txt` and `stderr.txt`; `inputs/`
contains the pre-run artifact snapshot and `outputs/` contains only files created or changed by the
command. The in-memory output limit does not truncate these on-disk stream files.


## Standalone CLI

The `bio_tools` executable wraps the same installer, status, and command-runner APIs for shell use:

```sh
bio_tools install opendde
bio_tools status-quick opendde
bio_tools status-full opendde
bio_tools metadata opendde
bio_tools run opendde -- --help

bio_tools list-quick
bio_tools list-full

bio_tools uninstall opendde
```

It uses `$BIO_TOOLS_ROOT`, or `./.bio_tools` when unset; `--root <directory>` overrides both.

`status-quick` inspects installation markers, executables, and required assets without launching the
tool. `status-full` also runs the tool's help/version probe and imports Torch or JAX where applicable
to report its compute device. The corresponding list commands are `list-quick` and `list-full`; the
older `status` and `list` commands remain aliases for the full variants. `run` resolves an installed
console entry point inside that managed environment, so it does not require the tool on `PATH`. Tools
that only expose a Python module or checkout script still need a tool-specific library invocation.


## Example uses
- Building a GUI (Web or native) to these tools
- Setting up an API to programmatically interface.


## Python bindings

The `python/` package builds an ABI3 wheel with PyO3 and maturin, published to PyPI as
`athanor_bio_tools`. It exposes the same process metadata, command runner, installer, and status
probes; see the examples above, and [the Rust docs](https://docs.rs/bio_tools) for details on the
underlying types.
