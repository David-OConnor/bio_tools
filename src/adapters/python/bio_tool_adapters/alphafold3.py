"""AlphaFold 3 structure prediction through its native JSON input.

Input format: https://github.com/google-deepmind/alphafold3/blob/v3.0.4/docs/input.md
CLI options: https://github.com/google-deepmind/alphafold3/blob/v3.0.4/run_alphafold.py
Performance and GPU settings: https://github.com/google-deepmind/alphafold3/blob/v3.0.4/docs/performance.md

A run has two steps, both in AlphaFold 3's own environment. `tool_scripts/alphafold3_prepare.py`
reads the input with AlphaFold 3's parser and fills in MSAs -- from the ColabFold MMseqs2 server
by default, so the ~630 GB genetic databases are not needed -- and `run_alphafold.py` then folds
what it wrote, with its data pipeline off unless the local databases were asked for.

The model parameters are the operator's: licensed by Google under the AlphaFold 3 Model
Parameters Terms of Use, and found in `process_executables/alphafold3/models` or
`ALPHAFOLD3_MODEL_DIR`.
"""

from __future__ import annotations

import datetime
import json
import os
import re
import shutil
import string
import tempfile
from pathlib import Path
from typing import Any

from . import (
    ToolExecutionError,
    ToolInputError,
    ToolUnavailable,
    catalog_spec,
    jax_device,
    readable_files,
    run_command,
    text,
    tool_fields,
    tool_python,
)
from .environments import bundle_path
from .field_processing import (
    boolean,
    choice,
    document_input,
    integer,
    json_list,
    molecule_boxes,
    safe_name,
)
from .status_check import CheckResult, ToolStatus, probe_command, require_gpu_status

SPEC = catalog_spec("alphafold3", fields=tool_fields("alphafold3"))

PREPARE = Path(__file__).resolve().parent / "tool_scripts" / "alphafold3_prepare.py"

# What each polymer calls a modification, by the molecule box's type.
_MODIFICATION_KEYS = {
    "protein": ("ptmType", "ptmPosition"),
    "dna": ("modificationType", "basePosition"),
    "rna": ("modificationType", "basePosition"),
}

# The docs allow only these letters in nucleic-acid chains.
_NUCLEOTIDES = {"dna": frozenset("ACGT"), "rna": frozenset("ACGU")}

_ENTITY_ID = re.compile(r"[A-Z]+")

# `run_alphafold.py`'s HMMER flags, by binary.
_HMMER = ("jackhmmer", "nhmmer", "hmmalign", "hmmsearch", "hmmbuild")

# Upstream's Dockerfile sets these for every run; a 7.x GPU needs the second flag too.
_XLA_FLAGS = "--xla_gpu_enable_triton_gemm=false"
_XLA_FLAG_CC7 = "--xla_disable_hlo_passes=custom-kernel-fusion-rewriter"

_PARAMETERS_URL = "https://storage.googleapis.com/alphafold3/af3.bin.zst"


def from_boxes(payload: dict[str, Any]) -> str:
    """The molecule boxes as one AlphaFold 3 input, for "Set parameters here"."""

    boxes = molecule_boxes(
        payload,
        allowed_kinds=frozenset({"protein", "dna", "rna", "ligand", "ion"}),
        allow_ids=True,
        allow_count=True,
        id_count_matches=True,
        maximum_id_length=4,
    )
    taken = {name for box in boxes for name in box.ids}
    free = (name for name in _chain_names() if name not in taken)

    sequences: list[dict[str, Any]] = []
    for box in boxes:
        ids = box.ids or [next(free) for _ in range(box.count)]
        for name in ids:
            if not _ENTITY_ID.fullmatch(name):
                raise ToolInputError(
                    f'Entity ID "{name}" must be uppercase letters, such as A or AB.'
                )
        entity: dict[str, Any] = {"id": ids[0] if len(ids) == 1 else ids}

        if box.kind == "ion":
            # Ions are ligands to AlphaFold 3, named by their CCD code.
            entity["ccdCodes"] = [box.ion.removeprefix("CCD_")]
            sequences.append({"ligand": entity})
            continue
        if box.kind == "ligand":
            entity.update(_ligand(box.ligand, box.chain))
            sequences.append({"ligand": entity})
            continue
        if box.cyclic:
            raise ToolInputError(
                f'Molecule "{box.chain}": AlphaFold 3 does not support cyclic chains, since it '
                "accepts no bonds within or between polymers."
            )
        alphabet = _NUCLEOTIDES.get(box.kind)
        if alphabet and set(box.sequence) - alphabet:
            invalid = "".join(sorted(set(box.sequence) - alphabet))
            raise ToolInputError(
                f'Molecule "{box.chain}": a {box.kind.upper()} sequence uses only '
                f'{", ".join(sorted(alphabet))}, not {invalid}.'
            )
        entity["sequence"] = box.sequence
        if box.modifications:
            type_key, position_key = _MODIFICATION_KEYS[box.kind]
            entity["modifications"] = [
                {type_key: mod.residue.removeprefix("CCD_"), position_key: mod.position}
                for mod in box.modifications
            ]
        if box.kind == "protein" and (box.unpaired_msa_path or box.paired_msa_path):
            # AlphaFold 3 takes both MSAs or neither; the one not given is run empty.
            for key, path in (
                ("unpairedMsa", box.unpaired_msa_path),
                ("pairedMsa", box.paired_msa_path),
            ):
                if path:
                    entity[f"{key}Path"] = path
                else:
                    entity[key] = ""
        elif box.kind == "rna" and box.unpaired_msa_path:
            entity["unpairedMsaPath"] = box.unpaired_msa_path
        sequences.append({box.kind: entity})

    document: dict[str, Any] = {
        "name": safe_name(payload, default="af3-job"),
        "modelSeeds": _seeds(payload),
        "sequences": sequences,
    }
    bonds = json_list(payload, "bonded_atom_pairs", max_length=200_000, maximum=5_000)
    if bonds:
        document["bondedAtomPairs"] = bonds
    user_ccd = text(payload, "user_ccd", required=False, max_length=2_000_000)
    if user_ccd:
        document["userCCD"] = user_ccd
    document["dialect"] = "alphafold3"
    document["version"] = 4
    return json.dumps(document, indent=2)


def _chain_names():
    """A, B, ... Z, AA, BA, ...: AlphaFold 3's own order for IDs it assigns."""

    length = 1
    while True:
        for index in range(26**length):
            name = ""
            for _ in range(length):
                index, remainder = divmod(index, 26)
                name += string.ascii_uppercase[remainder]
            yield name
        length += 1


def _ligand(value: str, chain: str) -> dict[str, Any]:
    """A ligand box's text: CCD codes (`CCD_ATP`, or `CCD_NAG, CCD_BMA`) or a SMILES string."""

    value = value.strip()
    if value.upper().startswith("FILE_"):
        raise ToolInputError(
            f'Molecule "{chain}": AlphaFold 3 takes ligands as CCD codes, SMILES, or a '
            "user-provided CCD, not structure files."
        )
    parts = [part.strip() for part in value.split(",")]
    coded = [part.upper().startswith("CCD_") for part in parts]
    if all(coded):
        codes = [part[4:] for part in parts]
        if not all(codes):
            raise ToolInputError(f'Molecule "{chain}" has an empty CCD code.')
        return {"ccdCodes": codes}
    if any(coded):
        raise ToolInputError(
            f'Molecule "{chain}" mixes CCD codes with SMILES; a ligand is one or the other.'
        )
    return {"smiles": value}


def _seeds(payload: dict[str, Any]) -> list[int]:
    raw = text(payload, "model_seeds", required=False, max_length=400) or "1"
    seeds: list[int] = []
    for item in raw.split(","):
        item = item.strip()
        try:
            seed = int(item)
        except ValueError as exc:
            raise ToolInputError(f'model_seeds entry "{item}" is not an integer.') from exc
        if not 0 <= seed <= 4_294_967_295:
            raise ToolInputError(f'model_seeds entry "{item}" is out of range.')
        seeds.append(seed)
    if len(seeds) > 100:
        raise ToolInputError("model_seeds accepts at most 100 seeds.")
    return seeds


def _input_document(payload: dict[str, Any]) -> Any:
    """The input, checked for the shape AlphaFold 3 expects before a run is started for it.

    AlphaFold 3's own parser, run by the prepare step, is the authority; this only catches what
    can be said without it, so an obviously wrong document fails at once.
    """

    raw = text(payload, "input_json", max_length=5_000_000)
    try:
        document = json.loads(raw)
    except json.JSONDecodeError as exc:
        raise ToolInputError(
            f"input_json is not valid JSON: {exc.msg} at line {exc.lineno}."
        ) from exc

    if isinstance(document, list):
        # An AlphaFold Server job list, which AlphaFold 3 converts itself.
        if not document or not all(isinstance(job, dict) for job in document):
            raise ToolInputError("An AlphaFold Server input is a non-empty list of jobs.")
        if len(document) > 32:
            raise ToolInputError("input_json accepts at most 32 jobs per run.")
        return document
    if not isinstance(document, dict):
        raise ToolInputError(
            "input_json must be an AlphaFold 3 input object or an AlphaFold Server job list."
        )
    if document.get("dialect") != "alphafold3":
        raise ToolInputError('An AlphaFold 3 input needs "dialect": "alphafold3".')
    if document.get("version") not in (1, 2, 3, 4):
        raise ToolInputError('An AlphaFold 3 input needs "version" 1, 2, 3, or 4.')
    sequences = document.get("sequences")
    if not isinstance(sequences, list) or not sequences:
        raise ToolInputError("An AlphaFold 3 input needs a non-empty sequences list.")
    for entry in sequences:
        if not isinstance(entry, dict) or len(entry) != 1:
            raise ToolInputError("Each entry in sequences names exactly one entity.")
        kind = next(iter(entry))
        if kind not in ("protein", "rna", "dna", "ligand"):
            raise ToolInputError(
                f'Unsupported entity type "{kind}"; ions are ligands, e.g. '
                '{"ligand": {"id": "B", "ccdCodes": ["MG"]}}.'
            )
    seeds = document.get("modelSeeds")
    if not isinstance(seeds, list) or not seeds:
        raise ToolInputError("An AlphaFold 3 input needs at least one seed in modelSeeds.")
    return document


def _optional_integer(
    payload: dict[str, Any], name: str, *, minimum: int, maximum: int
) -> int | None:
    value = payload.get(name)
    if value is None or str(value).strip() == "":
        return None
    return integer(payload, name, default=minimum, minimum=minimum, maximum=maximum)


def _date(payload: dict[str, Any]) -> str:
    value = text(payload, "max_template_date", required=False, max_length=10) or "2021-09-30"
    try:
        datetime.date.fromisoformat(value)
    except ValueError as exc:
        raise ToolInputError("max_template_date must be a date, as YYYY-MM-DD.") from exc
    return value


def _buckets(payload: dict[str, Any]) -> str | None:
    raw = text(payload, "buckets", required=False, max_length=400)
    if not raw:
        return None
    try:
        values = [int(item) for item in raw.split(",")]
    except ValueError as exc:
        raise ToolInputError("buckets must be comma-separated integers.") from exc
    if not values or values[0] < 1 or any(b <= a for a, b in zip(values, values[1:])):
        raise ToolInputError("buckets must be positive and strictly increasing.")
    return ",".join(str(value) for value in values)


def _msa_server(payload: dict[str, Any]) -> str:
    host = str(payload.get("msa_server_url") or "https://api.colabfold.com").strip()
    if not re.fullmatch(r"https://[^\s]+", host):
        raise ToolInputError("msa_server_url must be an HTTPS URL.")
    return host.rstrip("/")


def _home() -> Path:
    """`process_executables/alphafold3`, or the desktop's chosen bundle directory."""

    return bundle_path("alphafold3")


def _runner() -> Path:
    configured = os.getenv("ALPHAFOLD3_RUNNER")
    runner = Path(configured).expanduser() if configured else _home() / "source" / "run_alphafold.py"
    if not runner.is_file():
        raise ToolUnavailable(
            f"AlphaFold 3's run_alphafold.py is not at {runner}. Run `python install_tools.py "
            "alphafold3` (setup_system.sh does this through bio_tools), or set ALPHAFOLD3_RUNNER."
        )
    return runner


def model_dir() -> Path:
    configured = os.getenv("ALPHAFOLD3_MODEL_DIR")
    return Path(configured).expanduser() if configured else _home() / "models"


def has_parameters(directory: Path) -> bool:
    """Whether `directory` holds a file `alphafold3.model.params` loads as parameters."""

    if not directory.is_dir():
        return False
    return any(
        path.is_file()
        and re.fullmatch(r".*(\.[0-9]+\.bin(\.zst)?|\.bin(\.zst)?(\.[0-9]+)?)", path.name)
        for path in directory.iterdir()
    )


def _parameters_missing(directory: Path) -> str:
    return (
        f"AlphaFold 3's model parameters are not in {directory}. Google publishes them at "
        f"{_PARAMETERS_URL} under the AlphaFold 3 Model Parameters Terms of Use (non-commercial "
        "organisations only); once you have accepted those terms, put af3.bin.zst there or set "
        "ALPHAFOLD3_MODEL_DIR."
    )


def database_dir() -> Path:
    configured = os.getenv("ALPHAFOLD3_DATABASE_DIR")
    return Path(configured).expanduser() if configured else _home() / "databases"


def _hmmer_arguments() -> list[str]:
    """`--<binary>_binary_path` for each HMMER tool the data pipeline runs."""

    bundled = _home() / "hmmer" / "bin"
    arguments = []
    for binary in _HMMER:
        path = bundled / binary
        found = str(path) if path.is_file() else shutil.which(binary)
        if not found:
            raise ToolUnavailable(
                f"The AlphaFold 3 data pipeline needs HMMER's {binary}, which is neither in "
                f"{bundled} nor on PATH. Reinstall AlphaFold 3 to build it."
            )
        arguments.append(f"--{binary}_binary_path={found}")
    return arguments


def _environment(attention: str, unified_memory: bool) -> dict[str, str]:
    """The XLA settings upstream's Dockerfile and performance docs give, where not overridden."""

    flags = os.environ.get("XLA_FLAGS") or _XLA_FLAGS
    if attention == "xla" and _XLA_FLAG_CC7 not in flags:
        flags = f"{flags} {_XLA_FLAG_CC7}"
    environment = {"XLA_FLAGS": flags}
    if unified_memory:
        environment |= {
            "XLA_PYTHON_CLIENT_PREALLOCATE": "false",
            "TF_FORCE_UNIFIED_MEMORY": "true",
            "XLA_CLIENT_MEM_FRACTION": "3.2",
        }
    else:
        environment |= {
            "XLA_PYTHON_CLIENT_PREALLOCATE": os.environ.get(
                "XLA_PYTHON_CLIENT_PREALLOCATE", "true"
            ),
            "XLA_CLIENT_MEM_FRACTION": os.environ.get("XLA_CLIENT_MEM_FRACTION", "0.95"),
        }
    return environment


def _summary(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {}
    return value if isinstance(value, dict) else {}


def run(payload: dict[str, Any]) -> dict[str, Any]:
    payload = document_input(
        payload, "input_json", from_boxes=from_boxes, max_length=5_000_000
    )
    document = _input_document(payload)

    msa_source = choice(payload, "msa_source", ("colabfold", "none", "databases"), "colabfold")
    host = _msa_server(payload)
    run_inference = boolean(payload, "run_inference", True)
    samples = integer(payload, "num_diffusion_samples", default=5, minimum=1, maximum=64)
    recycles = integer(payload, "num_recycles", default=10, minimum=1, maximum=100)
    num_seeds = _optional_integer(payload, "num_seeds", minimum=2, maximum=1_000)
    conformer_iterations = _optional_integer(
        payload, "conformer_max_iterations", minimum=1, maximum=100_000
    )
    attention = choice(
        payload, "flash_attention_implementation", ("triton", "cudnn", "xla"), "triton"
    )
    backend = choice(payload, "jax_backend", ("gpu", "cpu"), "gpu")
    if backend == "cpu" and attention != "xla":
        raise ToolInputError("CPU inference needs the XLA attention implementation.")
    gpu_device = integer(payload, "gpu_device", default=0, minimum=0, maximum=63)
    max_template_date = _date(payload)
    buckets = _buckets(payload)
    resolve_msa_overlaps = boolean(payload, "resolve_msa_overlaps", True)

    python = tool_python("alphafold3", "ALPHAFOLD3_PYTHON")
    runner = _runner()
    models = model_dir()
    if run_inference and not has_parameters(models):
        raise ToolUnavailable(_parameters_missing(models))
    databases = database_dir()
    if msa_source == "databases" and not databases.is_dir():
        raise ToolUnavailable(
            f"The AlphaFold 3 data pipeline needs the genetic databases (~630 GB; see "
            f"fetch_databases.sh in the AlphaFold 3 checkout) in {databases}, or "
            "ALPHAFOLD3_DATABASE_DIR set to them. Choose the ColabFold MSA source instead to "
            "run without them."
        )
    # Only AlphaFold 3's data pipeline and model do anything the prepare step has not.
    launch_alphafold = run_inference or msa_source == "databases"

    with tempfile.TemporaryDirectory(prefix="bio-web-af3-") as temporary:
        workdir = Path(temporary)
        input_path = workdir / "input.json"
        prepared = workdir / "inputs"
        summary_path = workdir / "prepared.json"
        output = workdir / "output"
        input_path.write_text(json.dumps(document, indent=2), encoding="utf-8")

        prepare = [
            python,
            str(PREPARE),
            str(input_path),
            str(prepared),
            str(summary_path),
            "--msa",
            msa_source,
            "--host",
            host,
        ]
        if num_seeds is not None:
            prepare += ["--num-seeds", str(num_seeds)]
        try:
            result = run_command(
                prepare, cwd=workdir, artifacts=[prepared, summary_path, input_path]
            )
        except ToolExecutionError as exc:
            failure = _summary(summary_path)
            if failure.get("kind") == "input":
                raise ToolInputError(f"AlphaFold 3 refused the input: {failure['error']}") from exc
            if failure.get("kind") == "msa_server":
                raise ToolExecutionError(
                    f"The MSA server ({host}) did not return alignments: {failure['error']}"
                ) from exc
            raise
        summary = _summary(summary_path)
        jobs = summary.get("jobs") or []

        structures: list[Path] = []
        failed: list[str] = []
        if launch_alphafold:
            command = [
                python,
                str(runner),
                f"--input_dir={prepared}",
                f"--output_dir={output}",
                f"--model_dir={models}",
                f"--run_data_pipeline={str(msa_source == 'databases').lower()}",
                f"--run_inference={str(run_inference).lower()}",
                f"--num_diffusion_samples={samples}",
                f"--num_recycles={recycles}",
                f"--flash_attention_implementation={attention}",
                f"--jax_backend={backend}",
                f"--gpu_device={gpu_device}",
                f"--max_template_date={max_template_date}",
                # A custom pairing laid out in the unpaired MSAs is kept row for row only if
                # nothing is deduplicated against the (query-only) paired MSA.
                "--resolve_msa_overlaps="
                + str(resolve_msa_overlaps and not summary.get("paired")).lower(),
                f"--fix_standalone_glycans={str(boolean(payload, 'fix_standalone_glycans')).lower()}",
                f"--save_embeddings={str(boolean(payload, 'save_embeddings')).lower()}",
                f"--save_distogram={str(boolean(payload, 'save_distogram')).lower()}",
                "--compress_large_output_files="
                + str(boolean(payload, "compress_large_output_files")).lower(),
                f"--jax_compilation_cache_dir={_home() / 'jax_cache'}",
            ]
            if num_seeds is not None:
                command.append(f"--num_seeds={num_seeds}")
            if conformer_iterations is not None:
                command.append(f"--conformer_max_iterations={conformer_iterations}")
            if buckets is not None:
                command.append(f"--buckets={buckets}")
            if msa_source == "databases":
                command += [f"--db_dir={databases}", *_hmmer_arguments()]
            (_home() / "jax_cache").mkdir(parents=True, exist_ok=True)

            result = run_command(
                command,
                cwd=workdir,
                env=_environment(attention, boolean(payload, "unified_memory")),
                artifacts=[output, prepared],
            )
            generated = readable_files(output)
            structures = sorted(output.rglob("*_model.cif")) + sorted(
                output.rglob("*_model.cif.zst")
            )
            if run_inference:
                failed = [
                    job["name"]
                    for job in jobs
                    if not any((output / job["sanitised"]).glob("*_model.cif*"))
                ]
        else:
            generated = readable_files(prepared)

    if run_inference and not structures:
        raise ToolExecutionError(
            "AlphaFold 3 exited without writing a structure. See the run log for details."
        )

    response: dict[str, Any] = {
        "status": "completed",
        "input": {"json": document},
        "generated_files": generated,
        "msa_source": msa_source,
        **result,
    }
    warnings = list(summary.get("warnings") or [])
    if failed:
        warnings.append(
            "AlphaFold 3 wrote no structure for "
            + ", ".join(f'"{name}"' for name in failed)
            + ". See the run log for details."
        )
    if msa_source == "none":
        warnings.append(
            "Chains without MSAs of their own ran MSA-free, which is usually much less accurate."
        )
    if warnings:
        response["warnings"] = warnings
    return response


def check_status() -> ToolStatus:
    try:
        python = tool_python("alphafold3", "ALPHAFOLD3_PYTHON")
        _runner()
    except ToolUnavailable as exc:
        return ToolStatus(CheckResult.NOT_INSTALLED, str(exc))
    code, output = probe_command(
        [
            python,
            "-c",
            "import importlib.metadata as m, alphafold3.model.model; "
            "print('AlphaFold 3', m.version('alphafold3'))",
        ],
        timeout=300,
    )
    if code is None:
        return ToolStatus(CheckResult.NOT_INSTALLED, output)
    if code != 0:
        return ToolStatus(CheckResult.ERROR, output.splitlines()[-1][:200] if output else "")
    models = model_dir()
    if not has_parameters(models):
        return ToolStatus(CheckResult.ERROR, _parameters_missing(models))
    status = ToolStatus(CheckResult.PASS, output.splitlines()[-1][:200])
    return require_gpu_status(status, jax_device(python), "AlphaFold 3")
