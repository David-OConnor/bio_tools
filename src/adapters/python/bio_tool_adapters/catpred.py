"""CatPred enzyme kinetics: kcat, Km and Ki with an uncertainty estimate.

One request is one CSV of reactions, which is what CatPred itself takes: a row
is an enzyme sequence, the substrate (or inhibitor) SMILES, and a sequence ID
that must name exactly one sequence across the file. The form offers the same
row three ways -- boxes for a single reaction, the CSV typed in, or the CSV
uploaded -- and every one of them ends up as that CSV.
"""

from __future__ import annotations

import csv
import io
import json
import os
import re
import tempfile
from pathlib import Path
from typing import Any

from . import (
    PROCESS_EXECUTABLES,
    ToolExecutionError,
    ToolInputError,
    ToolUnavailable,
    catalog_spec,
    readable_files,
    run_command,
    tool_fields,
    tool_python,
    torch_device,
)
from .environments import environment_python
from .field_processing import (
    choice,
    document_input,
    molecule_boxes,
    safe_name,
    text,
)
from .status_check import CheckResult, ToolStatus, probe_python_package

SPEC = catalog_spec("catpred", fields=tool_fields("catpred"))
RUNNER = Path(__file__).resolve().parent / "tool_scripts" / "catpred_inference.py"

# One checkpoint ensemble per parameter, selected by name rather than inferred.
PARAMETERS = ("kcat", "km", "ki")
# CatPred trains on the 20 canonical amino acids and rejects anything else
# outright (`catpred.inference.service._VALID_AAS`), so this is its alphabet
# rather than the wider one the structure predictors accept.
AMINO_ACIDS = frozenset("ACDEFGHIKLMNPQRSTVWY")
# The three columns CatPred requires of an input CSV, in its own order. Any
# other column is carried through to the predictions untouched, which is how
# the substrate names in its demo files survive into the output.
REQUIRED_COLUMNS = ("SMILES", "sequence", "pdbpath")
# A ceiling on one submission rather than a limit of the model: CatPred's own
# service caps a request at 1000 rows, and each row costs an ESM-2 embedding
# and ten model evaluations.
MAX_ROWS = 200
MAX_SEQUENCE = 20_000
MAX_SMILES = 2_000
# Where the checkpoint archive puts each parameter's ensemble, relative to the
# checkout's `capsule_data`. `production` is what CatPred's own demos and web
# service predict with (ten models per parameter); the reproduce checkpoints
# beside it belong to the paper's per-seed experiments, and only some of them
# are usable on their own, so they are a fallback and never the first choice.
CHECKPOINT_LAYOUTS = (
    "capsule_data/data/pretrained/production/{parameter}",
    "checkpoint_links/{parameter}",
    "capsule_data/data/pretrained/reproduce_checkpoints/{parameter}/seed0",
)


def checkout_root() -> Path:
    """The CatPred checkout the installer cloned, which carries the weights."""

    configured = os.getenv("CATPRED_HOME")
    if configured:
        return Path(configured).expanduser()
    runner = os.getenv("CATPRED_RUNNER")
    if runner:
        return Path(runner).expanduser().parent
    return PROCESS_EXECUTABLES / "CatPred"


def checkpoint_dir(parameter: str) -> Path:
    """The ensemble directory to predict `parameter` with.

    An explicit CATPRED_CHECKPOINT_DIR wins and is read as a directory holding
    one subdirectory per parameter, which is the layout the installer builds;
    otherwise the checkout's own copy of the downloaded archive is searched.
    A directory only counts when it actually holds checkpoints, so a partial
    download is reported here rather than deep inside the model loader.
    """

    candidates: list[Path] = []
    configured = os.getenv("CATPRED_CHECKPOINT_DIR")
    if configured:
        root = Path(configured).expanduser()
        candidates.extend([root / parameter, root])
    checkout = checkout_root()
    candidates.extend(
        checkout / layout.format(parameter=parameter) for layout in CHECKPOINT_LAYOUTS
    )
    for candidate in candidates:
        if candidate.is_dir() and any(candidate.rglob("model.pt")):
            return candidate.resolve()
    raise ToolUnavailable(
        f"No CatPred {parameter} checkpoints were found. Reinstall CatPred to "
        "download the checkpoint archive, or set CATPRED_CHECKPOINT_DIR to a "
        "directory holding a kcat, km and ki ensemble."
    )


def parse_sequence(value: str, where: str) -> str:
    sequence = re.sub(r"\s+", "", value).upper()
    if not sequence:
        raise ToolInputError(f"{where} needs an enzyme sequence.")
    if len(sequence) > MAX_SEQUENCE:
        raise ToolInputError(f"{where} sequence is longer than {MAX_SEQUENCE:,} residues.")
    unsupported = set(sequence) - AMINO_ACIDS
    if unsupported:
        raise ToolInputError(
            f"{where} sequence contains {''.join(sorted(unsupported))}, which CatPred "
            "does not accept: use the 20 canonical amino acids, without gaps, "
            "ambiguity codes or chain separators."
        )
    return sequence


def parse_smiles(value: str, where: str) -> str:
    smiles = "".join(str(value).split())
    if not smiles:
        raise ToolInputError(f"{where} needs a SMILES string.")
    if len(smiles) > MAX_SMILES:
        raise ToolInputError(f"{where} SMILES is too long.")
    return smiles


def parse_rows(document: str) -> list[dict[str, str]]:
    """CatPred's input CSV, validated the way CatPred validates it.

    `pdbpath` is an identifier rather than a file: it names the sequence whose
    embedding is cached, so it may be left out here and filled in per unique
    sequence, but it can never name two different sequences in one file.
    """

    document = document.lstrip("﻿").strip()
    if not document:
        raise ToolInputError("Provide at least one reaction.")
    try:
        reader = csv.DictReader(io.StringIO(document), strict=True)
        raw_rows = list(reader)
    except csv.Error as exc:
        raise ToolInputError(f"That CSV could not be read: {exc}") from exc
    columns = [name for name in (reader.fieldnames or []) if name]
    missing = [
        name
        for name in ("SMILES", "sequence")
        if name not in columns
    ]
    if missing:
        raise ToolInputError(
            "A CatPred CSV needs SMILES and sequence columns, and a pdbpath column "
            "unless the IDs are to be filled in here; missing: "
            + ", ".join(missing)
            + "."
        )
    if not raw_rows:
        raise ToolInputError("That CSV has a header but no reactions.")
    if len(raw_rows) > MAX_ROWS:
        raise ToolInputError(f"At most {MAX_ROWS} reactions are supported per job.")

    rows: list[dict[str, str]] = []
    names: dict[str, str] = {}
    generated: dict[str, str] = {}
    for offset, raw in enumerate(raw_rows, start=2):
        if any(key is None for key in raw) or any(
            value is None for value in raw.values()
        ):
            raise ToolInputError(f"Row {offset} has a different number of columns than the header.")
        where = f"Row {offset}"
        sequence = parse_sequence(str(raw.get("sequence") or ""), where)
        row = {
            "SMILES": parse_smiles(str(raw.get("SMILES") or ""), where),
            "sequence": sequence,
            "pdbpath": str(raw.get("pdbpath") or "").strip(),
        }
        if not row["pdbpath"]:
            row["pdbpath"] = generated.setdefault(
                sequence, f"seq_{len(generated) + 1:03d}"
            )
        # Stricter than CatPred, which only strips the value: this ID is also
        # what a molecule box holds when a CSV is read back into the form, and
        # a box takes one unspaced name.
        if len(row["pdbpath"]) > 60 or any(
            character.isspace() or character in "\\/," for character in row["pdbpath"]
        ):
            raise ToolInputError(
                f'{where} sequence ID "{row["pdbpath"]}" must be a name of up to 60 '
                "characters, without spaces, slashes or commas."
            )
        known = names.setdefault(row["pdbpath"], sequence)
        if known != sequence:
            raise ToolInputError(
                f'{where} reuses the sequence ID "{row["pdbpath"]}" for a different '
                "sequence. Each ID must name exactly one sequence, because it is what "
                "CatPred caches that sequence's embedding under."
            )
        for column in columns:
            if column not in REQUIRED_COLUMNS:
                row[column] = str(raw.get(column) or "").strip()
        rows.append(row)
    return rows


def from_boxes(payload: dict[str, Any]) -> str:
    """The one reaction the boxes describe, written as CatPred's input CSV."""

    parameter = choice(payload, "parameter", PARAMETERS, "kcat")
    boxes = molecule_boxes(
        payload,
        allowed_kinds=frozenset({"protein", "ligand"}),
        maximum=12,
        allow_ids=True,
        maximum_id_length=60,
    )
    proteins = [box for box in boxes if box.kind == "protein"]
    ligands = [box for box in boxes if box.kind == "ligand"]
    if len(proteins) != 1:
        raise ToolInputError(
            "CatPred scores one enzyme at a time here: give exactly one protein box, "
            "or enter a CSV to score several reactions in one run."
        )
    if not ligands:
        raise ToolInputError(
            "Add the inhibitor as a ligand box."
            if parameter == "ki"
            else "Add at least one substrate as a ligand box."
        )
    if parameter != "kcat" and len(ligands) > 1:
        subject = "inhibitor" if parameter == "ki" else "substrate"
        raise ToolInputError(
            f"A {parameter} prediction is for one {subject}: keep a single ligand box. "
            "Only kcat is predicted for a whole substrate set."
        )
    sequence = parse_sequence(proteins[0].sequence, "The enzyme")
    # CatPred's own CSVs join a reaction's substrates with ".", the way a
    # multi-component SMILES is written; its web app does the same before
    # sending a kcat row.
    smiles = ".".join(
        parse_smiles(ligand.ligand, "Each substrate") for ligand in ligands
    )
    row = {
        "SMILES": smiles,
        "sequence": sequence,
        "pdbpath": proteins[0].chain,
    }
    # A ligand box named by the reader labels the row, as the "Substrate"
    # column in CatPred's demo files does. Boxes left with the automatic
    # single-letter ID have not been named, and label nothing.
    labels = [ligand.chain for ligand in ligands if len(ligand.chain) > 1]
    if labels:
        row["Substrate"] = " + ".join(labels)
    buffer = io.StringIO(newline="")
    writer = csv.DictWriter(buffer, fieldnames=list(row), lineterminator="\n")
    writer.writeheader()
    writer.writerow(row)
    return buffer.getvalue()


def prepare(payload: dict[str, Any]) -> dict[str, Any]:
    payload = document_input(
        payload, "input_csv", from_boxes=from_boxes, max_length=2_000_000
    )
    parameter = choice(payload, "parameter", PARAMETERS, "kcat")
    return {
        "parameter": parameter,
        "rows": parse_rows(text(payload, "input_csv", max_length=2_000_000)),
        "device": choice(payload, "device", {"auto", "cpu", "cuda"}, "auto"),
        "checkpoint_dir": str(checkpoint_dir(parameter)),
    }


def run(payload: dict[str, Any]) -> dict[str, Any]:
    name = safe_name(payload, default="catpred-job")
    request = prepare(payload)
    python = tool_python("catpred", "CATPRED_PYTHON")
    with tempfile.TemporaryDirectory(prefix="bio-web-catpred-") as temporary:
        workdir = Path(temporary)
        input_path = workdir / "request.json"
        output_path = workdir / name
        input_path.write_text(json.dumps(request, indent=2), encoding="utf-8")
        result = run_command(
            [
                python,
                str(RUNNER),
                "--input",
                str(input_path),
                "--work-dir",
                str(workdir / "work"),
                "--output-dir",
                str(output_path),
            ],
            cwd=workdir,
            artifacts=[input_path, output_path],
        )
        try:
            summary = json.loads(
                (output_path / "summary.json").read_text(encoding="utf-8")
            )
        except (OSError, json.JSONDecodeError) as exc:
            raise ToolExecutionError(
                "CatPred finished without a readable summary.json. See the run log "
                "for details."
            ) from exc
        generated = readable_files(output_path)
    return {
        "status": "completed",
        "job_name": name,
        "parameter": request["parameter"],
        # The predictions themselves are in `summary`, and the CSV they were
        # read out of is the run's own output; neither is repeated here.
        "summary": summary,
        "generated_files": generated,
        **result,
    }


def check_status() -> ToolStatus:
    python = environment_python("catpred")
    if not python.is_file():
        return ToolStatus(
            CheckResult.NOT_INSTALLED,
            "Run `python install_tools.py catpred` to install CatPred.",
        )
    status = probe_python_package(python, "catpred", "catpred.inference")
    if status.result != CheckResult.PASS:
        return status
    missing = []
    for parameter in PARAMETERS:
        try:
            checkpoint_dir(parameter)
        except ToolUnavailable:
            missing.append(parameter)
    if missing:
        return ToolStatus(
            CheckResult.NOT_INSTALLED,
            "CatPred is installed, but no checkpoints were found for: "
            + ", ".join(missing)
            + ". Reinstall it to download the checkpoint archive.",
        )
    status.device = torch_device(python)
    return status
