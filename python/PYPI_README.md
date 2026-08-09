# bio-tools

Python bindings for the `bio_tools` Rust library. The wheel exposes shared process metadata, Rust-backed command execution, installation, and status probes for native and web applications.

```python
from pathlib import Path
import bio_tools

root = Path("process_executables")
installer = bio_tools.Installer(root)
installer.install(bio_tools.Tool("opendde"))

quick = bio_tools.Tool("opendde").status_quick(root)
full = bio_tools.Tool("opendde").status_full(root)
print(quick.result, quick.detail)
print(full.result, full.detail, full.device)
print(installer.list_quick())

result = bio_tools.Command(
    ["opendde", "predict", "input.yaml"],
    cwd=Path("work"),
    timeout=600,
    run_log_dir=root / "run_logs",
    run_name="opendde",
).run()
print(result.stdout)
print(result.run_log_dir)
```

Configured run logs retain the exact argument vector, optional stdin, complete
stdout and stderr, and before/after artifact copies in a unique directory for
each invocation. Output returned to Python may stay bounded without truncating
the durable stream files.

Assets and source checkouts are placed under `process_executables`; isolated Python and Conda environments are placed under `process_executables/python_envs`.
