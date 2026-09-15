"""ESMC sequence representations and masked-marginal substitution scoring."""

from __future__ import annotations

import json
import re
import tempfile
from pathlib import Path
from typing import Any

from . import (
    ToolExecutionError,
    ToolInputError,
    catalog_spec,
    readable_files,
    run_command,
    tool_fields,
    tool_python,
    torch_device,
)
from .environments import environment_python
from .field_processing import (
    boolean,
    choice,
    document_input,
    integer,
    json_list,
    safe_name,
    text,
)
from .status_check import CheckResult, ToolStatus, probe_command, probe_python_package

SPEC = catalog_spec("esmc", fields=tool_fields("esmc"))
RUNNER = Path(__file__).resolve().parent / "tool_scripts" / "esmc_inference.py"
MODELS = {"biohub/ESMC-300M": 30, "biohub/ESMC-600M": 36, "biohub/ESMC-6B": 80}
AMINO_ACIDS = frozenset("ACDEFGHIKLMNPQRSTVWYXBZUO")


def parse_sequences(raw: str) -> list[dict[str, str]]:
    """FASTA records or one wrapped sequence; never truncate or split a protein."""
    lines = [line.strip() for line in raw.splitlines() if line.strip()]
    if not lines:
        raise ToolInputError("Provide a protein sequence or FASTA records.")
    if not lines[0].startswith(">") and any(line.startswith(">") for line in lines):
        raise ToolInputError(
            "FASTA input must start with a header; do not mix bare sequences and FASTA records."
        )
    records: list[dict[str, str]] = []
    for line in lines:
        if line.startswith(">"):
            header = line[1:].strip()
            if not header or len(header) > 200:
                raise ToolInputError("FASTA headers must contain 1–200 characters.")
            records.append({"id": header, "sequence": ""})
        else:
            if not records:
                records.append({"id": "protein_1", "sequence": ""})
            records[-1]["sequence"] += re.sub(r"\s+", "", line).upper()
    if len(records) > 26:
        raise ToolInputError(
            "At most 26 independent protein sequences are supported per job."
        )
    seen = set()
    for record in records:
        sequence = record["sequence"]
        if record["id"] in seen:
            raise ToolInputError("FASTA headers must be unique.")
        seen.add(record["id"])
        if not 1 <= len(sequence) <= 2046:
            raise ToolInputError(
                "Each protein must contain 1–2046 residues (2048 tokens including BOS/EOS)."
            )
        if set(sequence) - AMINO_ACIDS:
            raise ToolInputError(
                "Use amino-acid letters, including X, B, Z, U or O; gaps, masks and chain separators are not accepted."
            )
    return records


def from_boxes(payload: dict[str, Any]) -> str:
    boxes = json_list(payload, "sequence_molecules", maximum=26, max_length=100_000)
    if not boxes:
        raise ToolInputError("Add at least one protein.")
    records = []
    for index, box in enumerate(boxes, 1):
        if not isinstance(box, dict) or box.get("type") != "protein":
            raise ToolInputError("ESMC accepts protein sequences only.")
        if box.get("count", 1) != 1:
            raise ToolInputError(
                "ESMC analyzes one independent sequence per box; copies are not supported."
            )
        if any(
            box.get(key)
            for key in (
                "cyclic",
                "modifications",
                "msa",
                "paired_msa_path",
                "unpaired_msa_path",
                "templates_path",
            )
        ):
            raise ToolInputError(
                "ESMC accepts unmodified linear sequences without MSAs or templates."
            )
        name = box.get("id") or f"protein_{index}"
        seq = box.get("sequence")
        if not isinstance(name, str) or any(c in name for c in "\r\n>,"):
            raise ToolInputError(
                "Each protein needs a single text ID without newlines or commas."
            )
        if not isinstance(seq, str) or ">" in seq:
            raise ToolInputError(
                "Each protein box needs a sequence, without FASTA headers."
            )
        records.append(f">{name}\n{seq}\n")
    return "".join(records)


def prepare(payload: dict[str, Any]) -> dict[str, Any]:
    payload = document_input(
        payload, "input_fasta", from_boxes=from_boxes, max_length=100_000
    )
    records = parse_sequences(text(payload, "input_fasta", max_length=100_000))
    model = choice(payload, "model", MODELS, "biohub/ESMC-300M")
    layer = integer(
        payload, "hidden_layer", default=-1, minimum=-1, maximum=MODELS[model]
    )
    mutations = []
    raw = text(payload, "mutations", required=False, max_length=10_000)
    for value in re.split(r"[\s,;]+", raw):
        if not value:
            continue
        match = re.fullmatch(
            r"([ACDEFGHIKLMNPQRSTVWY])(\d+)([ACDEFGHIKLMNPQRSTVWY])", value.upper()
        )
        if not match or len(records) != 1:
            raise ToolInputError(
                "Substitutions require exactly one protein and notation such as A42G (one-based)."
            )
        wt, pos, mutant = match.groups()
        position = int(pos)
        if (
            not 1 <= position <= len(records[0]["sequence"])
            or records[0]["sequence"][position - 1] != wt
        ):
            raise ToolInputError(
                f"{value}: reference residue does not match the input sequence."
            )
        if wt == mutant:
            raise ToolInputError(
                f"{value}: reference and substituted residues must differ."
            )
        mutation = {
            "mutation": f"{wt}{position}{mutant}",
            "position": position,
            "wild_type": wt,
            "mutant": mutant,
        }
        if mutation not in mutations:
            mutations.append(mutation)
    if len(mutations) > 100:
        raise ToolInputError("At most 100 substitutions are supported per job.")
    return {
        "records": records,
        "model": model,
        "device": choice(payload, "device", {"auto", "cpu", "cuda"}, "auto"),
        "precision": choice(
            payload, "precision", {"default", "fp32", "bf16"}, "default"
        ),
        "batch_size": integer(payload, "batch_size", default=1, minimum=1, maximum=8),
        "save_embeddings": boolean(payload, "save_embeddings", True),
        "save_logits": boolean(payload, "save_logits", False),
        "hidden_layer": layer,
        "mutations": mutations,
    }


def run(payload: dict[str, Any]) -> dict[str, Any]:
    name = safe_name(payload, default="esmc-demo")
    request = prepare(payload)
    python = tool_python("esmc", "ESMC_PYTHON")
    with tempfile.TemporaryDirectory(prefix="bio-web-esmc-") as temporary:
        workdir = Path(temporary)
        input_path = workdir / "input.json"
        output_path = workdir / name
        input_path.write_text(json.dumps(request, indent=2), encoding="utf-8")
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
            artifacts=[input_path, output_path],
        )
        try:
            summary = json.loads(
                (output_path / "summary.json").read_text(encoding="utf-8")
            )
        except (OSError, json.JSONDecodeError) as exc:
            raise ToolExecutionError(
                "ESMC finished without a readable summary.json. See the run log for details."
            ) from exc
        generated = readable_files(output_path, maximum_files=100)
    return {
        "status": "completed",
        "summary": summary,
        "generated_files": generated,
        **result,
    }


def check_status() -> ToolStatus:
    python = environment_python("esmc")
    if not python.is_file():
        return ToolStatus(
            CheckResult.NOT_INSTALLED,
            "Run `python install_tools.py esmc` to install ESMC.",
        )
    status = probe_python_package(python, "esm", "esm.models.esmc")
    if status.result == CheckResult.PASS:
        code, output = probe_command(
            [
                str(python),
                "-c",
                "from esm.models.esmc import EsmcForMaskedLM, EsmcTokenizer",
            ]
        )
        if code != 0:
            return ToolStatus(
                CheckResult.ERROR,
                (output or "Reinstall ESMC to update its API.")[-200:],
            )
        status.device = torch_device(python)
    return status
