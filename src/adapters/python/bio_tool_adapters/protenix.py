"""Protenix structure prediction through its native batch JSON input.

Input format: https://github.com/bytedance/Protenix/blob/main/docs/infer_json_format.md
CLI options: https://github.com/bytedance/Protenix/blob/main/runner/batch_inference.py
"""

from __future__ import annotations

import copy
import json
import os
import re
import tempfile
from pathlib import Path
from typing import Any

import bio_tools

from . import (
    ToolExecutionError,
    ToolInputError,
    ToolUnavailable,
    catalog_spec,
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
    document_input,
    integer,
    json_list,
    json_object,
    molecule_boxes,
    safe_name,
)
from .status_check import CheckResult, ToolStatus, probe_cli, require_gpu_status

SPEC = catalog_spec("protenix", fields=tool_fields("protenix"))

# docs/supported_models.md
MODELS = {
    "protenix-v2",
    "protenix_base_default_v1.0.0",
    "protenix_base_20250630_v1.0.0",
    "protenix_base_default_v0.5.0",
    "protenix_base_constraint_v0.5.0",
    "protenix_mini_esm_v0.5.0",
    "protenix_mini_ism_v0.5.0",
    "protenix_mini_default_v0.5.0",
    "protenix_tiny_default_v0.5.0",
}
# `protenix pred` asserts that only these accept --use_template and --use_rna_msa.
TEMPLATE_AND_RNA_MSA_MODELS = {
    "protenix-v2",
    "protenix_base_default_v1.0.0",
    "protenix_base_20250630_v1.0.0",
}
# The only checkpoint whose constraint embedders are enabled; every other model
# featurizes a `constraint` section and then never reads it.
CONSTRAINT_MODELS = {"protenix_base_constraint_v0.5.0"}

# What one chain is called in Protenix's own input, by the molecule box's type.
_POLYMERS = {
    "protein": "proteinChain",
    "dna": "dnaSequence",
    "rna": "rnaSequence",
}

_MODIFICATION_KEYS = {
    "protein": ("ptmType", "ptmPosition"),
    "dna": ("modificationType", "basePosition"),
    "rna": ("modificationType", "basePosition"),
}

_BUNDLED = "bio-tools://protenix/"


def from_boxes(payload: dict[str, Any]) -> str:
    """The molecule boxes as one Protenix job, for "Set parameters here".

    One job holding every box, which is what the boxes describe: a single
    system to fold. A run of several jobs is what the JSON mode is for.
    """

    boxes = molecule_boxes(
        payload,
        allowed_kinds=frozenset({"protein", "dna", "rna", "ligand", "ion"}),
        allow_ids=True,
        allow_count=True,
        id_count_matches=True,
    )
    sequences: list[dict[str, Any]] = []
    covalent_bonds: list[dict[str, Any]] = []
    for index, box in enumerate(boxes, start=1):
        entity: dict[str, Any] = {"count": box.count}
        if box.ids:
            entity["id"] = box.ids
        if box.kind == "ligand":
            entity["ligand"] = box.ligand
            sequences.append({"ligand": entity})
            continue
        if box.kind == "ion":
            entity["ion"] = box.ion.removeprefix("CCD_")
            sequences.append({"ion": entity})
            continue
        if box.cyclic:
            # Protenix has no cyclic flag. Its input docs name a head-to-tail
            # amide bond in `covalent_bonds` as the supported way to close a
            # cyclic peptide; polymer-polymer bonds between nucleotides are not
            # reliably handled, so those are refused rather than attempted.
            if box.kind != "protein":
                raise ToolInputError(
                    f'Molecule "{box.chain}": Protenix supports cyclic peptides only.'
                )
            if len(box.sequence) < 2:
                raise ToolInputError(
                    f'Molecule "{box.chain}" must have at least 2 residues to be cyclic.'
                )
            # Without copy indexes the bond is made within each copy in turn.
            covalent_bonds.append(
                {
                    "entity1": str(index),
                    "position1": str(len(box.sequence)),
                    "atom1": "C",
                    "entity2": str(index),
                    "position2": "1",
                    "atom2": "N",
                }
            )
        entity["sequence"] = box.sequence
        if box.modifications:
            type_key, position_key = _MODIFICATION_KEYS[box.kind]
            entity["modifications"] = [
                {
                    type_key: f"CCD_{mod.residue.removeprefix('CCD_')}",
                    position_key: mod.position,
                }
                for mod in box.modifications
            ]
        if box.kind == "protein":
            if box.paired_msa_path:
                entity["pairedMsaPath"] = box.paired_msa_path
            if box.unpaired_msa_path:
                entity["unpairedMsaPath"] = box.unpaired_msa_path
            if box.templates_path:
                entity["templatesPath"] = box.templates_path
        elif box.kind == "rna" and box.unpaired_msa_path:
            entity["unpairedMsaPath"] = box.unpaired_msa_path
        sequences.append({_POLYMERS[box.kind]: entity})

    job: dict[str, Any] = {
        "name": safe_name(payload, default="protenix-job"),
        "sequences": sequences,
    }
    covalent_bonds[:0] = json_list(payload, "covalent_bonds", max_length=500_000)
    if covalent_bonds:
        job["covalent_bonds"] = covalent_bonds
    constraint = json_object(payload, "constraint")
    if constraint:
        job["constraint"] = constraint
    return json.dumps([job], indent=2)


def _input_document(payload: dict[str, Any]) -> list[dict[str, Any]]:
    raw = text(payload, "input_json", max_length=1_000_000)
    try:
        document = json.loads(raw)
    except json.JSONDecodeError as exc:
        raise ToolInputError(
            f"input_json is not valid JSON: {exc.msg} at line {exc.lineno}."
        ) from exc

    if not isinstance(document, list) or not document:
        raise ToolInputError("input_json must be a non-empty top-level list of jobs.")
    if len(document) > 32:
        raise ToolInputError("input_json accepts at most 32 jobs per run.")
    names: set[str] = set()
    for index, job in enumerate(document, start=1):
        if not isinstance(job, dict):
            raise ToolInputError(f"Protenix job {index} must be a JSON object.")
        if not isinstance(job.get("name"), str) or not job["name"].strip():
            raise ToolInputError(f"Protenix job {index} needs a non-empty name.")
        name = job["name"]
        # Each job's results land in a directory of this name, beside the ERR
        # directory Protenix writes its failures to.
        if (
            name.casefold() == "err"
            or name in {".", ".."}
            or any(character in name for character in "/\\\0")
        ):
            raise ToolInputError(
                f'Protenix job name "{name}" must be a safe path component.'
            )
        if name in names:
            raise ToolInputError(f'Protenix job name "{name}" is duplicated.')
        names.add(name)

        model_seeds = job.get("modelSeeds")
        if model_seeds is not None:
            if not isinstance(model_seeds, list):
                raise ToolInputError(f'modelSeeds for job "{name}" must be a list.')
            if any(
                isinstance(seed, bool) or _seed(seed) is None for seed in model_seeds
            ):
                raise ToolInputError(
                    f'modelSeeds for job "{name}" must contain integer seeds '
                    "from 0 to 4294967295."
                )
        sequences = job.get("sequences")
        if not isinstance(sequences, list) or not sequences:
            raise ToolInputError(
                f'Protenix job "{name}" needs a non-empty sequences list.'
            )
        for entry in sequences:
            if not isinstance(entry, dict) or len(entry) != 1:
                raise ToolInputError(
                    f'Each entry in job "{name}"\'s sequences names exactly one entity.'
                )
        constraint = job.get("constraint")
        if constraint is not None and not isinstance(constraint, dict):
            raise ToolInputError(f'constraint for job "{name}" must be an object.')
    return document


def _seed(value: Any) -> int | None:
    if isinstance(value, str) and value.strip() == value and value.isdecimal():
        value = int(value)
    if isinstance(value, int) and 0 <= value <= 4_294_967_295:
        return value
    return None


def _seeds(payload: dict[str, Any]) -> str:
    raw = text(payload, "seeds", required=False, max_length=200).strip() or "101"
    seeds: list[str] = []
    for item in raw.split(","):
        item = item.strip()
        try:
            value = int(item)
        except ValueError as exc:
            raise ToolInputError(f'seeds entry "{item}" is not an integer.') from exc
        if not 0 <= value <= 4_294_967_295:
            raise ToolInputError(f'seeds entry "{item}" is out of range.')
        seeds.append(str(value))
    if len(seeds) > 32:
        raise ToolInputError("seeds accepts at most 32 values.")
    return ",".join(seeds)


def _choice(
    payload: dict[str, Any], field: str, choices: set[str], default: str
) -> str:
    value = str(payload.get(field) or default)
    if value not in choices:
        raise ToolInputError(f"Unsupported {field}: {value}.")
    return value


def _entities(document: list[dict[str, Any]]):
    """Every (job, entity type, entity) in the document."""

    for job in document:
        for entry in job["sequences"]:
            for kind, entity in entry.items():
                if isinstance(entity, dict):
                    yield job, kind, entity


def materialize_bundled(document: list[dict[str, Any]], directory: Path) -> None:
    """Write the presets' bundled alignments into the job directory.

    A preset names the official example MSAs by `bio-tools://protenix/...`
    reference so the same preset works on any runner; Protenix itself needs a
    file, so each reference becomes an absolute path to a copy made here.
    """

    written: dict[str, str] = {}
    for _job, _kind, entity in _entities(document):
        for key in ("pairedMsaPath", "unpairedMsaPath", "templatesPath"):
            reference = entity.get(key)
            if not isinstance(reference, str) or not reference.startswith(_BUNDLED):
                continue
            if reference not in written:
                try:
                    content = bio_tools.catalog_input_text("protenix", reference)
                except ValueError as exc:
                    raise ToolInputError(str(exc)) from exc
                path = directory / "bundled" / reference.removeprefix(_BUNDLED)
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content, encoding="utf-8")
                written[reference] = str(path.resolve())
            entity[key] = written[reference]


def check_paths(
    document: list[dict[str, Any]],
    *,
    use_msa: bool,
    use_template: bool,
    use_rna_msa: bool,
) -> None:
    """Refuse file references Protenix would quietly not use.

    A missing MSA or template file makes Protenix search for one instead, and
    a path given while its feature is off is never read; either way the run
    would not use the file named here.
    """

    for job, kind, entity in _entities(document):
        name = job.get("name")
        if kind == "ligand":
            ligand = entity.get("ligand")
            if isinstance(ligand, str) and ligand.startswith("FILE_"):
                if not Path(ligand.removeprefix("FILE_")).is_file():
                    raise ToolInputError(
                        f'Job "{name}": ligand file {ligand.removeprefix("FILE_")!r} '
                        "is not a file on the compute node."
                    )
            continue
        checks = {
            "proteinChain": (
                ("pairedMsaPath", use_msa, "use_msa"),
                ("unpairedMsaPath", use_msa, "use_msa"),
                ("templatesPath", use_template, "use_template"),
            ),
            "rnaSequence": (("unpairedMsaPath", use_rna_msa, "use_rna_msa"),),
        }.get(kind, ())
        for key, enabled, switch in checks:
            path = entity.get(key)
            if path in (None, ""):
                continue
            if not enabled:
                raise ToolInputError(
                    f'Job "{name}" gives {key} for a {kind}, but {switch} is off, '
                    "so Protenix would ignore it."
                )
            if not isinstance(path, str) or not Path(path).is_file():
                raise ToolInputError(
                    f'Job "{name}": {key} {path!r} is not a file on the compute node.'
                )
        # The template search aligns against the protein's MSA, so a protein
        # with no template file of its own needs MSAs to get one.
        if (
            kind == "proteinChain"
            and use_template
            and not use_msa
            and not entity.get("templatesPath")
        ):
            raise ToolInputError(
                f'Job "{name}": searching for templates needs use_msa; turn it on '
                "or give the protein a templatesPath."
            )


def run(payload: dict[str, Any]) -> dict[str, Any]:
    payload = document_input(payload, "input_json", from_boxes=from_boxes)
    document = _input_document(payload)

    model = _choice(payload, "model", MODELS, "protenix-v2")
    seeds = _seeds(payload)
    samples = integer(payload, "samples", default=5, minimum=1, maximum=64)
    steps = integer(payload, "steps", default=200, minimum=1, maximum=1_000)
    cycles = integer(payload, "cycles", default=10, minimum=1, maximum=100)
    dtype = _choice(payload, "dtype", {"bf16", "fp32"}, "bf16")
    msa_server_mode = _choice(
        payload, "msa_server_mode", {"protenix", "colabfold"}, "protenix"
    )
    triatt_kernel = _choice(
        payload,
        "triatt_kernel",
        {"triattention", "cuequivariance", "deepspeed", "torch"},
        "cuequivariance",
    )
    trimul_kernel = _choice(
        payload, "trimul_kernel", {"cuequivariance", "torch"}, "cuequivariance"
    )

    use_msa = boolean(payload, "use_msa", True)
    use_template = boolean(payload, "use_template")
    use_rna_msa = boolean(payload, "use_rna_msa")
    use_seeds_in_json = boolean(payload, "use_seeds_in_json")
    if (use_template or use_rna_msa) and model not in TEMPLATE_AND_RNA_MSA_MODELS:
        raise ToolInputError(
            "use_template and use_rna_msa require one of these models: "
            + ", ".join(sorted(TEMPLATE_AND_RNA_MSA_MODELS))
            + "."
        )
    if use_rna_msa and not use_msa:
        raise ToolInputError("use_rna_msa requires use_msa to be enabled.")
    constrained = [job["name"] for job in document if job.get("constraint")]
    if constrained and model not in CONSTRAINT_MODELS:
        raise ToolInputError(
            f'Job "{constrained[0]}" has a constraint, which only '
            "protenix_base_constraint_v0.5.0 reads; other models ignore it."
        )
    if use_seeds_in_json and not document[0].get("modelSeeds"):
        raise ToolInputError(
            "use_seeds_in_json reads the first job's modelSeeds, and that job has none."
        )

    arguments = [
        "pred",
        "-i",
        "input.json",
        "-o",
        "output",
        "-n",
        model,
        "-s",
        seeds,
        "-e",
        str(samples),
        "-p",
        str(steps),
        "-c",
        str(cycles),
        "-d",
        dtype,
        "--use_default_params",
        str(boolean(payload, "use_default_params")).lower(),
        "--use_msa",
        str(use_msa).lower(),
        "--msa_server_mode",
        msa_server_mode,
        "--use_template",
        str(use_template).lower(),
        "--use_rna_msa",
        str(use_rna_msa).lower(),
        "--use_seeds_in_json",
        str(use_seeds_in_json).lower(),
        "--use_tfg_guidance",
        str(boolean(payload, "use_guidance")).lower(),
        "--need_atom_confidence",
        str(boolean(payload, "need_atom_confidence")).lower(),
        "--triatt_kernel",
        triatt_kernel,
        "--trimul_kernel",
        trimul_kernel,
        "--enable_cache",
        str(boolean(payload, "enable_cache", True)).lower(),
        "--enable_fusion",
        str(boolean(payload, "enable_fusion", True)).lower(),
        "--enable_tf32",
        str(boolean(payload, "enable_tf32", True)).lower(),
    ]

    # The document as the reader gave it: what the run is shown afterwards,
    # before bundled references become paths in a directory that is gone by then.
    portable = copy.deepcopy(document)
    with tempfile.TemporaryDirectory(prefix="bio-web-protenix-") as temporary:
        workdir = Path(temporary)
        input_path = workdir / "input.json"
        output_path = workdir / "output"
        materialize_bundled(document, workdir)
        check_paths(
            document,
            use_msa=use_msa,
            use_template=use_template,
            use_rna_msa=use_rna_msa,
        )
        input_path.write_text(json.dumps(document, indent=2), encoding="utf-8")
        result = run_command(
            [tool_script("protenix", "protenix", "PROTENIX_EXECUTABLE"), *arguments],
            cwd=workdir,
            artifacts=[output_path, input_path],
        )
        generated = readable_files(output_path)
        # Each job writes <name>/seed_<seed>/predictions/<name>_sample_<rank>.cif;
        # a job is counted as predicted on either half of that, so a later
        # change to the layout cannot turn a good run into a reported failure.
        structures = sorted(output_path.rglob("*.cif"))
        failed = [
            job["name"]
            for job in document
            if not any(
                output_path / job["name"] in structure.parents
                or structure.name.startswith(f"{job['name']}_")
                for structure in structures
            )
        ]
        errors = _error_reports(output_path)

    # `protenix pred` catches a failed job, logs it and exits 0, so a run that
    # produced nothing is only visible from what it left behind.
    log = f"{result.get('stdout') or ''}\n{result.get('stderr') or ''}"
    if not structures:
        raise ToolExecutionError(
            "Protenix exited without writing a structure. "
            + (_failure_detail(log, errors) or "See the run log for details.")
        )

    response: dict[str, Any] = {
        "status": "completed",
        "input": {"json": portable},
        "generated_files": generated,
        **result,
    }
    if failed:
        detail = _failure_detail(log, errors)
        response["warnings"] = [
            "Protenix wrote no structure for "
            + ", ".join(f'"{name}"' for name in failed)
            + "."
            + (f" {detail}" if detail else " See the run log for details.")
        ]
    return response


def _error_reports(output: Path) -> str:
    reports = sorted((output / "ERR").glob("*.txt")) if (output / "ERR").is_dir() else []
    return "\n".join(
        report.read_text(encoding="utf-8", errors="replace").strip()
        for report in reports
    )


def _failure_detail(log: str, errors: str) -> str:
    match = re.search(r"Run inference failed: (.+)", log)
    detail = match.group(1).strip() if match else errors.strip()
    if not detail:
        return ""
    return detail if len(detail) <= 2_000 else detail[:2_000] + "…"


# The reference data the installer fetches, with the size below which a file is
# certainly incomplete. Floors, not exact sizes: the CCD only grows, so this
# catches the failure that matters -- a download that stopped partway, which
# Protenix itself neither notices nor retries, and which makes every prediction
# die in the CCD lookup rather than saying anything about the file.
_REFERENCE_DATA = {
    "components.cif": 400_000_000,
    "components.cif.rdkit_mol.pkl": 100_000_000,
    "clusters-by-entity-40.txt": 1_000_000,
    "obsolete_release_date.csv": 1_000,
}


def _reference_data_problem() -> str | None:
    """What is wrong with the CCD reference data, if anything."""

    root = Path(os.environ.get("PROTENIX_ROOT_DIR") or Path.home()) / "common"
    for name, minimum in _REFERENCE_DATA.items():
        path = root / name
        if not path.is_file():
            return f"{path} is missing"
        size = path.stat().st_size
        if size < minimum:
            return f"{path} is {size} bytes, too small to be the whole file"
    return None


def check_status() -> ToolStatus:
    try:
        protenix = tool_script("protenix", "protenix", "PROTENIX_EXECUTABLE")
    except ToolUnavailable as exc:
        return ToolStatus(CheckResult.NOT_INSTALLED, str(exc))
    status = probe_cli([protenix])
    if status.result != CheckResult.PASS:
        return status
    status = require_gpu_status(
        status, torch_device(environment_python("protenix")), "Protenix"
    )
    if status.result != CheckResult.PASS:
        return status
    problem = _reference_data_problem()
    if problem:
        return ToolStatus(
            CheckResult.ERROR,
            f"Protenix is installed, but its reference data is incomplete: {problem}. "
            "Reinstall Protenix to fetch it again.",
            device=status.device,
        )
    return status
