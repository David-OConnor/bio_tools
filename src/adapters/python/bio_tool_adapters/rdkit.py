"""RDKit: molecule properties and atom-mapped reaction analysis.

RDKit is a library rather than a program, so there is no command line to mirror
here: what this adapter offers is the readers and calculations RDKit's own
documentation is organized around -- `SmilesMolSupplier` and `SDMolSupplier`
for input, `Descriptors`, `rdMolStandardize`, `rdMolHash`, `MurckoScaffold`,
`rdFingerprintGenerator` and `rdFMCS` for molecules, and `rdChemReactions` for
reactions. See https://www.rdkit.org/docs/GettingStartedInPython.html and
https://www.rdkit.org/docs/Cookbook.html .

The chemistry itself runs out of process, in `tool_scripts/rdkit_analysis.py`:
that is what gives an RDKit run the same log, output directory and downloadable
artifacts as every other tool here, and it keeps a molecule that sends RDKit
into a long ring perception out of the web worker. This module validates the
request, starts that process, and reads back what it wrote.

Every run leaves one CSV -- a row per molecule or reaction -- which is the file
the results page offers and renders, plus a summary holding the same values and
the nested detail a CSV cell cannot carry: a reaction's bond edits, and the
maximum common substructure of a whole set.
"""

from __future__ import annotations

import json
import os
import sys
import tempfile
from pathlib import Path
from typing import Any

from . import (
    ToolExecutionError,
    ToolInputError,
    ToolUnavailable,
    catalog_spec,
    readable_files,
    run_command,
    text,
    tool_fields,
    tool_tasks,
)
from .field_processing import boolean, choice, document_input, integer, safe_name
from .status_check import CheckResult, ToolStatus, probe_command

SPEC = catalog_spec(
    "rdkit",
    fields=tool_fields("rdkit"),
    tasks=tool_tasks("rdkit"),
)

RUNNER = Path(__file__).resolve().parent / "tool_scripts" / "rdkit_analysis.py"
# How the runner announces a problem with the input rather than a problem with
# itself. Only RDKit can tell whether a SMILES string is a molecule or a SMARTS
# pattern is a query, and RDKit is out of process, so the answer comes back as
# an exit message; this is what turns it back into the right kind of error.
INVALID_INPUT = "invalid input: "
MAX_INPUT_CHARACTERS = 200_000
# The rows returned inline with the API's answer. The whole table is the CSV,
# which is served as a file; this is what a client reads without downloading.
MAX_INLINE_ROWS = 200

FINGERPRINTS = {
    "none",
    "morgan",
    "rdkit",
    "atom_pair",
    "topological_torsion",
    "maccs",
}


def interpreter() -> str:
    """The Python that runs the analysis: this process's own, or an override.

    RDKit is a dependency of the application rather than a tool with an
    environment of its own, so the interpreter already running has it. An
    operator who keeps RDKit somewhere else names it with RDKIT_PYTHON.

    Never `.resolve()`: a uv venv puts a symlink to a shared base interpreter
    at bin/python, and that base interpreter has no pyvenv.cfg of its own.
    Invoking it by its resolved target makes CPython's site init treat it as a
    bare interpreter with none of this venv's packages on sys.path, so the
    `import rdkit` that works in this process fails in the child. Both paths
    here are already absolute -- `sys.executable` always, and RDKIT_PYTHON
    because it is rejected below if it is not.
    """

    configured = os.getenv("RDKIT_PYTHON")
    if configured:
        path = Path(configured)
        if not path.is_absolute() or not path.is_file():
            raise ToolUnavailable(
                f"RDKIT_PYTHON must be the absolute path of a Python interpreter: "
                f"{configured}"
            )
        return str(path)
    return sys.executable


def _from_boxes(payload: dict[str, Any]) -> str:
    """There is no parameter mode: the input is SMILES text or a file."""

    raise ToolInputError(
        "Provide SMILES text or upload a file: input_mode must be text or upload."
    )


def prepare(payload: dict[str, Any]) -> dict[str, Any]:
    """Everything a run needs, validated, before any process is started.

    The molecules themselves are not parsed here -- that needs RDKit, and RDKit
    is what the run is for. What is checked here is everything that would make
    the run meaningless whatever the molecules turned out to be.
    """

    task = choice(payload, "task", {"molecule", "reaction"}, "molecule")
    field = "smiles" if task == "molecule" else "reaction_smiles"
    payload = document_input(
        payload, field, from_boxes=_from_boxes, max_length=MAX_INPUT_CHARACTERS
    )
    request: dict[str, Any] = {
        "name": safe_name(payload, default="rdkit-demo"),
        "task": task,
        "input": text(payload, field, max_length=MAX_INPUT_CHARACTERS),
    }

    if task == "reaction":
        request.update(
            reaction_participants=boolean(payload, "reaction_participants", True),
            reaction_balance=boolean(payload, "reaction_balance", True),
            reaction_bond_edits=boolean(payload, "reaction_bond_edits", True),
        )
        if not any(
            (
                request["reaction_participants"],
                request["reaction_balance"],
                request["reaction_bond_edits"],
            )
        ):
            raise ToolInputError(
                "Choose at least one reaction analysis: participants, balance or "
                "bond edits."
            )
        return request

    request.update(
        descriptor_set=choice(
            payload, "descriptor_set", {"common", "all", "none"}, "common"
        ),
        standardize=boolean(payload, "standardize"),
        include_inchi=boolean(payload, "include_inchi"),
        hashes=boolean(payload, "hashes"),
        murcko_scaffold=boolean(payload, "murcko_scaffold"),
        ring_analysis=boolean(payload, "ring_analysis"),
        stereo=boolean(payload, "stereo"),
        substructure_smarts=text(
            payload, "substructure_smarts", required=False, max_length=2000
        ),
        find_mcs=boolean(payload, "find_mcs"),
        mcs_timeout=integer(payload, "mcs_timeout", default=20, minimum=1, maximum=600),
        fingerprint=choice(payload, "fingerprint", FINGERPRINTS, "none"),
        fingerprint_bits=integer(
            payload, "fingerprint_bits", default=2048, minimum=32, maximum=16_384
        ),
        fingerprint_radius=integer(
            payload, "fingerprint_radius", default=2, minimum=1, maximum=6
        ),
    )
    # A run that computes nothing still writes the canonical SMILES of every
    # molecule, which is a legitimate thing to ask RDKit for; there is no
    # combination here that produces an empty table.
    return request


def run(payload: dict[str, Any]) -> dict[str, Any]:
    request = prepare(payload)
    python = interpreter()

    with tempfile.TemporaryDirectory(prefix="bio-web-rdkit-") as temporary:
        workdir = Path(temporary)
        input_path = workdir / "request.json"
        output_path = workdir / request["name"]
        input_path.write_text(json.dumps(request, indent=2), encoding="utf-8")
        try:
            result = run_command(
                [
                    python,
                    str(RUNNER),
                    "--input",
                    str(input_path),
                    "--output-dir",
                    str(output_path),
                ],
                cwd=workdir,
                artifacts=[output_path, input_path],
            )
        except ToolExecutionError as exc:
            message = str(exc)
            marker = message.find(INVALID_INPUT)
            if marker < 0:
                raise
            raise ToolInputError(
                message[marker + len(INVALID_INPUT) :].strip()
            ) from exc
        try:
            summary = json.loads(
                (output_path / "summary.json").read_text(encoding="utf-8")
            )
        except (OSError, json.JSONDecodeError) as exc:
            raise ToolExecutionError(
                "RDKit finished without a readable summary.json. See the run log for "
                "details."
            ) from exc
        generated = readable_files(output_path)

    rows = summary.pop("rows", [])
    detail = {
        key: summary.pop(key)
        for key in ("reaction_analyses", "maximum_common_substructure")
        if key in summary
    }
    return {
        "status": "completed",
        "task": request["task"],
        "summary": summary,
        # The head of the table, for a client that wants the answer without
        # fetching the CSV; `summary["row_count"]` says how many there are.
        "rows": rows[:MAX_INLINE_ROWS],
        "rows_truncated": len(rows) > MAX_INLINE_ROWS,
        **detail,
        "warnings": summary.get("warnings") or [],
        "generated_files": generated,
        **result,
    }


# What the status probe asks the interpreter a run would use. Importing RDKit
# in *this* process would answer a different question, and answer it wrongly:
# a run is a child process, and a parent that can import RDKit is no evidence
# that the child can.
_PROBE = (
    "import sys\n"
    "from rdkit import Chem\n"
    "import rdkit\n"
    "inchi = 'yes'\n"
    "try:\n"
    "    inchi = 'yes' if Chem.MolToInchiKey(Chem.MolFromSmiles('CCO')) else 'no'\n"
    "except Exception:\n"
    "    inchi = 'no'\n"
    "print(rdkit.__version__, inchi)\n"
)


def check_status() -> ToolStatus:
    try:
        python = interpreter()
    except ToolUnavailable as exc:
        return ToolStatus(CheckResult.NOT_INSTALLED, str(exc))
    if not RUNNER.is_file():
        return ToolStatus(
            CheckResult.ERROR, f"The RDKit analysis script is missing: {RUNNER}"
        )

    code, output = probe_command([python, "-c", _PROBE])
    if code is None:
        return ToolStatus(
            CheckResult.NOT_INSTALLED, f"Could not run {python}: {output}"
        )
    if code != 0:
        return ToolStatus(
            CheckResult.NOT_INSTALLED,
            f"RDKit is not importable in {python}, which is the interpreter a run "
            f"uses: {(output or '').strip()[-200:]}",
        )

    parts = (output or "").split()
    version = parts[0] if parts else "?"
    detail = f"Imported rdkit {version} in {python}"
    # InChI is a build option and the form offers it, so the status says
    # whether this build can answer for it.
    if len(parts) > 1 and parts[1] == "no":
        detail = f"{detail}; this build has no InChI support"
    return ToolStatus(CheckResult.PASS, detail)
