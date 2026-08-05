# bio-tools

Python bindings for the `bio_tools` Rust library. The wheel exposes the shared installer and status probes used by native and web applications.

```python
from pathlib import Path
import bio_tools

root = Path("process_executables")
installer = bio_tools.Installer(root)
installer.install(bio_tools.Tool("opendde"))

status = bio_tools.Tool("opendde").status(root)
print(status.result, status.detail, status.device)
```

Assets and source checkouts are placed under `process_executables`; isolated Python and Conda environments are placed under `process_executables/python_envs`.
