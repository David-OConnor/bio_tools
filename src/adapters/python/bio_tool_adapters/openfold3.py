"""OpenFold3 inference with the OpenBind-0 checkpoint."""

from __future__ import annotations

import json
import os
import re
import tempfile
from pathlib import Path
from typing import Any

from . import (
    ToolExecutionError,
    ToolInputError,
    ToolUnavailable,
    catalog_spec,
    model_cache_environment,
    readable_files,
    run_command,
    text,
    tool_fields,
    tool_script,
    torch_device,
)
from .environments import environment_python
from .field_processing import (
    boolean,
    choice,
    document_input,
    integer,
    molecule_boxes,
    safe_name,
)
from .status_check import CheckResult, ToolStatus, probe_python_package, require_gpu_status

SPEC = catalog_spec("openfold3", fields=tool_fields("openfold3"))

_TYPES = frozenset({"protein", "dna", "rna", "ligand", "ion"})
_MSA_PATHS = ("main_msa_file_paths", "paired_msa_file_paths")
_TEMPLATE_PATHS = ("template_alignment_file_path", "template_cif_paths")
_CHECKPOINT = "of3-ob-2025-06-30-174k.pt"


def from_boxes(payload: dict[str, Any]) -> str:
    boxes = molecule_boxes(
        payload,
        allowed_kinds=_TYPES,
        allow_ids=True,
        allow_count=True,
        require_ids=True,
        id_count_matches=True,
    )
    chains = []
    for box in boxes:
        chain: dict[str, Any] = {
            "molecule_type": "ligand" if box.kind == "ion" else box.kind,
            "chain_ids": box.ids,
        }
        if box.kind == "ion":
            chain["ccd_codes"] = box.ion.removeprefix("CCD_")
        elif box.kind == "ligand":
            if box.ligand.startswith("CCD_"):
                chain["ccd_codes"] = box.ligand.removeprefix("CCD_")
            else:
                chain["smiles"] = box.ligand
        else:
            chain["sequence"] = box.sequence
            if box.cyclic:
                raise ToolInputError(f'Molecule "{box.chain}": cyclic chains are not supported.')
            if box.modifications:
                chain["non_canonical_residues"] = {
                    str(mod.position): mod.residue.removeprefix("CCD_")
                    for mod in box.modifications
                }
            if box.unpaired_msa_path:
                chain["main_msa_file_paths"] = box.unpaired_msa_path
            if box.paired_msa_path:
                chain["paired_msa_file_paths"] = box.paired_msa_path
            if box.templates_path:
                chain["template_alignment_file_path"] = box.templates_path
        chains.append(chain)

    name = safe_name(payload, default="openfold3-job")
    return json.dumps({"queries": {name: {"chains": chains}}}, indent=2)


def _document(
    payload: dict[str, Any], msa_source: str, use_templates: bool
) -> dict[str, Any]:
    try:
        document = json.loads(text(payload, "input_json", max_length=1_000_000))
    except json.JSONDecodeError as exc:
        raise ToolInputError(f"Invalid query JSON at line {exc.lineno}: {exc.msg}.") from exc

    queries = document.get("queries") if isinstance(document, dict) else None
    if not isinstance(queries, dict) or not 1 <= len(queries) <= 32:
        raise ToolInputError("OpenFold3 input needs a queries object with 1 to 32 named queries.")

    names = {"msas", "mappings", "templates"}
    for name, query in queries.items():
        if (
            not re.fullmatch(r"[A-Za-z0-9_-][A-Za-z0-9_.-]{0,79}", name)
            or name.casefold() in names
        ):
            raise ToolInputError(f"Invalid or duplicate OpenFold3 query name {name!r}.")
        names.add(name.casefold())
        chains = query.get("chains") if isinstance(query, dict) else None
        if not isinstance(chains, list) or not chains or not all(isinstance(c, dict) for c in chains):
            raise ToolInputError(f'Query "{name}" needs a non-empty list of chain objects.')
        if msa_source == "none":
            query["use_msas"] = False

        ids: set[str] = set()
        for chain in chains:
            chain_ids = chain.get("chain_ids")
            chain_ids = [chain_ids] if isinstance(chain_ids, str) else chain_ids
            if not isinstance(chain_ids, list) or not chain_ids:
                raise ToolInputError(f'Query "{name}": each chain needs chain_ids.')
            for chain_id in chain_ids:
                if not isinstance(chain_id, str) or not chain_id.strip() or chain_id in ids:
                    raise ToolInputError(f'Query "{name}": chain IDs must be unique and nonempty.')
                ids.add(chain_id)

            for field in (*_MSA_PATHS, *_TEMPLATE_PATHS, "sdf_file_path"):
                value = chain.get(field)
                if not value:
                    continue
                is_msa = field in _MSA_PATHS
                if is_msa and (msa_source != "precomputed" or query.get("use_msas") is False):
                    raise ToolInputError(f'Query "{name}": {field} requires precomputed MSAs.')
                if field in _TEMPLATE_PATHS and not use_templates:
                    raise ToolInputError(f'Query "{name}": enable templates to use {field}.')
                if field == "template_alignment_file_path" and msa_source == "colabfold":
                    raise ToolInputError(f'Query "{name}": ColabFold replaces template alignments.')
                paths = value if isinstance(value, list) else [value]
                for value in paths:
                    path = Path(value) if isinstance(value, str) else None
                    if path is None or not path.is_absolute() or not (
                        path.is_file() or (is_msa and path.is_dir())
                    ):
                        raise ToolInputError(
                            f'Query "{name}": {field} needs an existing absolute path '
                            f"on the compute node ({value!r})."
                        )
                    if path.suffix.lower() == ".npz" or (
                        is_msa and path.is_dir()
                        and any(entry.suffix.lower() == ".npz" for entry in path.iterdir())
                    ):
                        raise ToolInputError(
                            f'Query "{name}": supply text MSAs (.a3m or .sto) or raw templates. '
                            "OpenFold3 loads NumPy caches with pickle enabled."
                        )

    return document


def _runner() -> str:
    return tool_script("openfold3", "run_openfold", "OPENFOLD3_EXECUTABLE")


def _checkpoint() -> Path:
    checkpoint = Path(model_cache_environment()["OPENFOLD_CACHE"]) / _CHECKPOINT
    if not checkpoint.is_file():
        raise ToolUnavailable(f"Missing the OpenBind-0 checkpoint at {checkpoint}. Install OpenFold3.")
    return checkpoint.resolve()


def run(payload: dict[str, Any]) -> dict[str, Any]:
    payload = document_input(payload, "input_json", from_boxes=from_boxes)
    msa_source = choice(payload, "msa_source", ("colabfold", "precomputed", "none"), "colabfold")
    use_templates = boolean(payload, "use_templates")
    document = _document(payload, msa_source, use_templates)
    samples = integer(payload, "samples", default=1, minimum=1, maximum=64)
    seeds = integer(payload, "num_model_seeds", default=1, minimum=1, maximum=32)

    config: dict[str, Any] = {"output_writer_settings": {"structure_format": "cif"}}
    if boolean(payload, "low_memory", True):
        config["model_update"] = {"presets": ["predict", "low_mem"]}
        config["data_module_args"] = {"num_workers": 0}
    if server_url := os.environ.get("OPENFOLD3_MSA_SERVER_URL"):
        config["msa_computation_settings"] = {"server_url": server_url}

    with tempfile.TemporaryDirectory(prefix="bio-web-openfold3-") as temporary:
        workdir = Path(temporary)
        query_path = workdir / "input.json"
        config_path = workdir / "runner.yml"
        output = workdir / "output"
        query_path.write_text(json.dumps(document, indent=2), encoding="utf-8")
        config_path.write_text(json.dumps(config, indent=2), encoding="utf-8")
        result = run_command(
            [
                _runner(), "predict", "--query-json", str(query_path),
                "--inference-ckpt-path", str(_checkpoint()),
                "--runner-yaml", str(config_path), "--output-dir", str(output),
                "--use-msa-server", str(msa_source == "colabfold").lower(),
                "--use-templates", str(use_templates).lower(),
                "--num-diffusion-samples", str(samples), "--num-model-seeds", str(seeds),
            ],
            cwd=workdir,
            artifacts=[output, query_path, config_path],
        )
        structures = sorted(output.glob("*/seed_*/*_model.cif"))
        if not structures:
            raise ToolExecutionError("OpenFold3 wrote no predicted structures. See the run log.")
        generated = readable_files(output)
        finished = {path.relative_to(output).parts[0] for path in structures}

    response = {
        "status": "completed",
        "input": {"json": document},
        "generated_files": generated,
        "msa_source": msa_source,
        **result,
    }
    warnings = []
    if failed := sorted(document["queries"].keys() - finished):
        warnings.append("OpenFold3 wrote no structure for " + ", ".join(failed) + ".")
    if msa_source == "none":
        warnings.append("The model ran without MSAs. Prediction accuracy may be lower.")
    if warnings:
        response["warnings"] = warnings

    return response


def check_status() -> ToolStatus:
    try:
        _runner()
        _checkpoint()
    except ToolUnavailable as exc:
        return ToolStatus(CheckResult.NOT_INSTALLED, str(exc))

    python = environment_python("openfold3")
    status = probe_python_package(python, "openfold3", "openfold3.entry_points.experiment_runner")
    if status.result != CheckResult.PASS:
        return status

    return require_gpu_status(status, torch_device(python), "OpenFold3")
