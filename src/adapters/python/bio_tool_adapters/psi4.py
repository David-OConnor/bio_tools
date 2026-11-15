"""Run a Psithon input or PsiAPI script with the managed Psi4.

Psithon runs through Psi4's own `psi4` command, preprocessor included. PsiAPI
runs as a Python script. Both run in-process under a small runner that records
Psi4's variables, frequencies and final geometry when the input finishes.
Psithon and PsiAPI are Python: an input can do anything Python can.
"""

from __future__ import annotations

import json
import os
import tempfile
from pathlib import Path
from typing import Any

from . import ToolExecutionError, ToolUnavailable, catalog_spec, run_command
from .environments import environment_path
from .field_processing import document_input, text
from .psi4_input import (
    FIELDS,
    MAX_INPUT,
    PSIAPI,
    from_boxes,
    input_format,
    memory_mib,
    threads,
)
from .psi4_input import to_boxes as to_boxes

SPEC = catalog_spec("psi4", fields=list(FIELDS.values()))

# Runs inside Psi4's environment. argv[1] is the job description.
_RUNNER = '''\
import atexit
import csv
import json
import math
import runpy
import sys
from pathlib import Path

job = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
summary = Path(job["summary"])

import numpy
import psi4
from psi4.driver import driver
from psi4.driver.qcdb.vib import filter_nonvib, filter_omega_to_real

# frequency() and hessian() analyse vibrations through this function.
analyses = []
_vibanal_wfn = driver.vibanal_wfn


def _recorded_vibanal_wfn(*args, **kwargs):
    analysis = _vibanal_wfn(*args, **kwargs)
    analyses.append(analysis)
    return analysis


driver.vibanal_wfn = _recorded_vibanal_wfn


def _finite(value):
    value = float(value)
    return value if math.isfinite(value) else None


def _array(matrix):
    try:
        values = numpy.asarray(matrix.to_array(dense=True), dtype=float)
    except Exception:
        return None
    if values.size > 1000:
        return None
    return numpy.where(numpy.isfinite(values), values, None).tolist()


def _modes(analysis):
    analysis = filter_nonvib(analysis)
    frequencies = filter_omega_to_real(analysis["omega"].data)
    intensities = analysis.get("IR_intensity")
    modes = []
    for index, frequency in enumerate(frequencies):
        intensity = None if intensities is None else _finite(intensities.data[index])
        modes.append({
            "mode": index + 1,
            "symmetry": str(analysis["gamma"].data[index] or ""),
            "frequency_cm1": float(frequency),
            "imaginary": bool(frequency < 0),
            "ir_intensity_km_mol": intensity,
        })
    return modes


def _summarize():
    try:
        result = {
            "variables": {
                name: _finite(value)
                for name, value in sorted(psi4.core.scalar_variables().items())
            },
            "array_variables": {
                name: values
                for name, matrix in sorted(psi4.core.array_variables().items())
                if (values := _array(matrix)) is not None
            },
        }
        if analyses:
            result["vibrational_modes"] = modes = _modes(analyses[-1])
            with open(summary / "frequencies.csv", "w", newline="", encoding="utf-8") as handle:
                writer = csv.DictWriter(handle, fieldnames=list(modes[0]) if modes else ["mode"])
                writer.writeheader()
                writer.writerows(modes)
        try:
            molecule = psi4.core.get_active_molecule()
        except Exception:
            molecule = None
        if molecule is not None and molecule.natom():
            molecule.save_xyz_file(str(summary / "geometry.xyz"), False)
        text = json.dumps(result, allow_nan=False)
        (summary / "summary.json").write_text(text, encoding="utf-8")
    except Exception as error:
        print(f"Could not summarize the Psi4 run: {error!r}", file=sys.stderr)


# Registered after Psi4's own exit handlers, so this runs before they clean up.
atexit.register(_summarize)

if job["format"] == "psiapi":
    psi4.set_output_file(job["output"], False)
    psi4.set_num_threads(job["threads"], quiet=True)
    psi4.set_memory(job["memory"], quiet=True)
    psi4.core.IOManager.shared_object().set_default_path(job["scratch"])
    sys.argv = [job["input"]]
    sys.path.insert(0, str(Path(job["input"]).parent))
    runpy.run_path(job["input"], run_name="__main__")
else:
    sys.argv = [
        job["launcher"],
        "--nthread", str(job["threads"]),
        "--memory", job["memory"],
        "--scratch", job["scratch"],
        job["input"],
        job["output"],
    ]
    runpy.run_path(job["launcher"], run_name="__main__")
'''


def _invocation() -> tuple[list[str], Path, dict[str, str]]:
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
    # UTF-8 mode: Psi4's CLI reads the input with the platform's default encoding.
    command = [str(python), "-I", "-X", "utf8"]
    return command, launcher, {"CONDA_PREFIX": str(prefix), "PATH": path}


def _files(directory: Path, skip: Path | None = None) -> list[str]:
    return sorted(
        path.relative_to(directory).as_posix()
        for path in directory.rglob("*")
        if path.is_file() and path != skip
    )


def run(payload: dict[str, Any]) -> dict[str, Any]:
    payload = document_input(
        payload, "psi4_input", from_boxes=from_boxes,
        upload_field="psi4_input_file", max_length=MAX_INPUT,
    )
    document = text(payload, "psi4_input", max_length=MAX_INPUT)
    language = input_format(payload)
    thread_count = threads(payload)
    memory = memory_mib(payload)

    command, launcher, environment = _invocation()
    with tempfile.TemporaryDirectory(prefix="bio-tools-psi4-") as directory:
        root = Path(directory)
        # Psi4 writes its results in `psi4`, and its scratch files outside it.
        work, scratch, summary = root / "psi4", root / "scratch", root / "summary"
        for folder in (work, scratch, summary):
            folder.mkdir()
        source = work / ("input.py" if language == PSIAPI else "input.dat")
        source.write_text(document + "\n", encoding="utf-8")
        runner = root / "runner.py"
        runner.write_text(_RUNNER, encoding="utf-8")
        job = root / "job.json"
        job.write_text(json.dumps({
            "format": language,
            "launcher": str(launcher),
            "input": str(source),
            "output": str(work / "output.dat"),
            "scratch": str(scratch),
            "summary": str(summary),
            "threads": thread_count,
            "memory": f"{memory} MiB",
        }), encoding="utf-8")

        result = run_command(
            [*command, str(runner), str(job)],
            cwd=work,
            env={**environment, "PSI_SCRATCH": str(scratch)},
            artifacts=[work, summary],
        )
        try:
            computed = json.loads((summary / "summary.json").read_text(encoding="utf-8"))
        except (OSError, ValueError) as exc:
            raise ToolExecutionError("Psi4 finished without a readable summary.") from exc
        generated = _files(work, skip=source) + [
            f"summary/{name}" for name in _files(summary)
        ]

    energy = computed["variables"].get("CURRENT ENERGY")
    return {
        "status": "completed",
        "input_format": language,
        "threads": thread_count,
        "memory_mib": float(memory),
        **({"energy_hartree": energy} if energy is not None else {}),
        **computed,
        "generated_files": generated,
        **result,
    }
