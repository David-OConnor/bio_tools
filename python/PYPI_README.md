# bio-tools

Python bindings for the `bio_tools` Rust library. The wheel exposes shared process metadata, Rust-backed command execution, installation, and status probes for native and web applications.

```python
from pathlib import Path
import bio_tools

root = Path("process_executables")
installer = bio_tools.Installer(root)
installer.install(bio_tools.Tool("opendde"))

status = bio_tools.Tool("opendde").status(root)
print(status.result, status.detail, status.device)

result = bio_tools.Command(
    ["opendde", "predict", "input.yaml"],
    cwd=Path("work"),
    timeout=600,
).run()
print(result.stdout)
```

Assets and source checkouts are placed under `process_executables`; isolated Python and Conda environments are placed under `process_executables/python_envs`.
