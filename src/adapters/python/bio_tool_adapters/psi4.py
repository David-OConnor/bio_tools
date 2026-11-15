"""Run declarative molecular calculations through the managed Psi4 CLI."""

from __future__ import annotations

import json
import os
import tempfile
from pathlib import Path
from typing import Any

from . import ToolExecutionError, ToolUnavailable, catalog_spec, run_command
from .environments import environment_path
from .field_processing import document_input, text
from .psi4_input import FIELDS, MAX_INPUT, from_boxes, runtime_settings
from .psi4_input import to_boxes as to_boxes

SPEC = catalog_spec("psi4", fields=list(FIELDS.values()))

# Psithon can contain Python. Only this fixed script and parsed data literals
# reach the executable, and the CLI preprocessor is disabled.
_JOB = """\
import csv
import json
import math
from pathlib import Path

import psi4
from psi4.driver.qcdb.vib import filter_nonvib, filter_omega_to_real

molecule = psi4.geometry(settings["geometry"])
if settings["molecule_name"]:
    molecule.set_name(settings["molecule_name"])
psi4.set_options(settings["options"])
model = settings["method"] + "/" + settings["basis"]
calculation = settings["calculation"]
result = {}

if calculation in {"optimize", "optimize_frequency"}:
    energy = psi4.optimize(model, molecule=molecule)
    molecule.save_xyz_file("optimized.xyz", True)
    if calculation == "optimize_frequency":
        result["optimized_energy_hartree"] = float(energy)

if calculation in {"frequency", "optimize_frequency"}:
    kwargs = {}
    if settings["frequency_derivative"] != "auto":
        kwargs["dertype"] = int(settings["frequency_derivative"])
    energy, wfn = psi4.frequency(model, molecule=molecule, return_wfn=True, **kwargs)
    analysis = filter_nonvib(wfn.frequency_analysis)
    frequencies = filter_omega_to_real(analysis["omega"].data)
    ir = analysis.get("IR_intensity")
    modes = []
    for index, frequency in enumerate(frequencies):
        intensity = float(ir.data[index]) if ir is not None else None
        modes.append({
            "mode": index + 1,
            "symmetry": str(analysis["gamma"].data[index]),
            "frequency_cm1": float(frequency),
            "imaginary": bool(frequency < 0),
            "ir_intensity_km_mol": intensity if intensity is not None and math.isfinite(intensity) else None,
        })
    result["vibrational_modes"] = modes
    with open("frequencies.csv", "w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=[
            "mode", "symmetry", "frequency_cm1", "imaginary", "ir_intensity_km_mol",
        ])
        writer.writeheader()
        writer.writerows(modes)
elif calculation == "energy":
    energy = psi4.energy(model, molecule=molecule)

result["energy_hartree"] = float(energy)
Path("energy-hartree.txt").write_text(str(result["energy_hartree"]) + "\\n", encoding="ascii")
Path("summary.json").write_text(json.dumps(result, allow_nan=False), encoding="ascii")
"""


def _invocation() -> tuple[list[str], dict[str, str]]:
    prefix = environment_path("psi4").absolute()
    if os.name == "nt":
        scripts = prefix / "Scripts"
        python = prefix / "python.exe"
        launcher = scripts / "psi4-script.py"
        paths = [prefix, prefix / "Library" / "bin", scripts]
    else:
        scripts = prefix / "bin"
        python = scripts / "python"
        launcher = scripts / "psi4"
        paths = [scripts]
    if not (python.is_file() and launcher.is_file()):
        raise ToolUnavailable(
            f"Install Psi4 through bio_tools. Expected its environment at {prefix}."
        )
    path = os.pathsep.join(str(directory) for directory in paths)
    if inherited := os.environ.get("PATH"):
        path += os.pathsep + inherited
    return [str(python), "-I", str(launcher)], {"CONDA_PREFIX": str(prefix), "PATH": path}


def run(payload: dict[str, Any]) -> dict[str, Any]:
    payload = document_input(
        payload, "psi4_input", from_boxes=from_boxes,
        upload_field="psi4_input_file", max_length=MAX_INPUT,
    )
    document = text(payload, "psi4_input", max_length=MAX_INPUT)
    settings = runtime_settings(document)
    calculation = settings["calculation"]
    generated = ["psi4.out", "energy-hartree.txt", "summary.json", "input.dat"]
    if calculation in {"optimize", "optimize_frequency"}:
        generated.append("optimized.xyz")
    if calculation in {"frequency", "optimize_frequency"}:
        generated.append("frequencies.csv")

    command, environment = _invocation()
    with tempfile.TemporaryDirectory(prefix="bio-tools-psi4-") as directory:
        work = Path(directory)
        (work / "scratch").mkdir()
        (work / "input.dat").write_text(document + "\n", encoding="utf-8")
        (work / "job.py").write_text(f"settings = {ascii(settings)}\n{_JOB}", encoding="ascii")
        result = run_command(
            [
                *command,
                "--skip-preprocessor",
                "--scratch", "scratch",
                "--nthread", settings["threads"],
                "--memory", f"{settings['memory_mib']} MiB",
                "job.py", "psi4.out",
            ],
            cwd=work,
            env=environment,
            artifacts=[Path(name) for name in ["job.py", *generated]],
        )
        try:
            summary = json.loads((work / "summary.json").read_text(encoding="ascii"))
        except (OSError, ValueError) as exc:
            raise ToolExecutionError("Psi4 did not write a readable calculation summary.") from exc

    return {
        "status": "completed",
        "calculation": calculation,
        "method": settings["method"],
        "basis": settings["basis"],
        "reference": settings["options"]["reference"],
        "charge": int(settings["charge"]),
        "multiplicity": int(settings["multiplicity"]),
        "threads": int(settings["threads"]),
        "memory_mib": float(settings["memory_mib"]),
        "generated_files": generated,
        **summary,
        **result,
    }
