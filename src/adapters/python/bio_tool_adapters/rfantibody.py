"""RFantibody antibody and nanobody design.

One request runs the whole pipeline: antibody-finetuned RFdiffusion docks a
backbone and rebuilds the requested CDR loops, ProteinMPNN designs sequences
for those loops, and antibody-finetuned RF2 predicts the resulting complex so
the designs can be filtered. The two tasks differ only in the framework they
build on -- a paired heavy/light scFv, or a heavy-chain-only VHH -- so the
light-chain fields belong to the antibody task alone.
"""

from __future__ import annotations

import os
import re
import tempfile
from pathlib import Path
from typing import Any

import bio_tools

from . import (
    PROCESS_EXECUTABLES,
    ToolInputError,
    ToolUnavailable,
    catalog_spec,
    tool_fields,
    tool_tasks,
    readable_files,
    run_command,
    text,
    tool_python,
    tool_script,
    torch_device,
)
from .environments import environment_python
from .field_processing import boolean, decimal, integer, safe_name
from .status_check import CheckResult, ToolStatus, probe_cli, require_gpu_status

SPEC = catalog_spec(
    "rfantibody",
    fields=tool_fields("rfantibody"),
    tasks=tool_tasks("rfantibody"),
)

# The CDR loops each task's framework has, keyed by the name the form uses and
# valued by the code RFantibody annotates them with. A VHH has no light chain,
# so naming an lcdr loop in the nanobody task is an input error rather than a
# silently dropped request.
_HEAVY_LOOPS = {"hcdr1": "H1", "hcdr2": "H2", "hcdr3": "H3"}
_LIGHT_LOOPS = {"lcdr1": "L1", "lcdr2": "L2", "lcdr3": "L3"}
_TASK_LOOPS: dict[str, dict[str, str]] = {
    "antibody": {**_HEAVY_LOOPS, **_LIGHT_LOOPS},
    "nanobody": dict(_HEAVY_LOOPS),
}

# The HLT frameworks RFantibody ships in scripts/examples/example_inputs, and
# the task each one belongs to. The third choice, "custom", is an HLT file
# built here from a Chothia-numbered upload.
_BUNDLED_FRAMEWORKS = {"hu-4D5-8_Fv": "antibody", "h-NbBCII10": "nanobody"}

# Checkpoints setup_system.sh downloads into the checkout, one per stage.
_STAGE_WEIGHTS = (
    "RFdiffusion_Ab.pt",
    "ProteinMPNN_v48_noise_0.2.pt",
    "RF2_ab.pt",
)

_CHAIN = re.compile(r"[A-Za-z0-9]\Z")
_RESIDUE = re.compile(r"-?\d+[A-Za-z]?\Z")
_LENGTH = re.compile(r"\d{1,3}(?:-\d{1,3})?\Z")

# RFdiffusion prints these once per design, but only when it was given
# hotspots. Both values are 0-dimensional torch tensors, so they land in the
# log as "tensor(5.2341)" rather than as bare numbers.
_CLOSEST_DISTANCE = re.compile(
    r"Overall min distance hotspot to designed loop:\s*(?:tensor\()?([\d.]+)"
)
_MEAN_DISTANCE = re.compile(
    r"Average min distance hotspot to designed loop:\s*(?:tensor\()?([\d.]+)"
)


def _root() -> Path:
    configured = os.getenv("RFANTIBODY_ROOT")
    return Path(configured) if configured else PROCESS_EXECUTABLES / "RFantibody"


def _stage_environment(python: str) -> dict[str, str]:
    """PATH with RFantibody's own bin/ ahead of everything else.

    Its console scripts do not run their inference scripts under the
    interpreter that launched them: each one shells out to `python <script>`
    and lets PATH decide which. Without this a stage would be run by whatever
    `python` the server process happens to see, which is not the environment
    holding torch, dgl, and the model code.
    """

    return {
        "PATH": os.pathsep.join([str(Path(python).parent), os.environ.get("PATH", "")])
    }


def _task(payload: dict[str, Any]) -> str:
    task = str(payload.get("task") or "antibody")
    if task not in {option.value for option in SPEC.tasks}:
        raise ToolInputError(f"Unsupported task: {task}.")
    return task


def _framework(payload: dict[str, Any], task: str) -> str:
    value = text(payload, "framework", required=False, max_length=50)
    expected = next(name for name, owner in _BUNDLED_FRAMEWORKS.items() if owner == task)
    if not value:
        return expected
    if value == "custom":
        return value
    belongs_to = _BUNDLED_FRAMEWORKS.get(value)
    if belongs_to is None:
        raise ToolInputError(f"Unsupported framework: {value}.")
    if belongs_to != task:
        raise ToolInputError(
            f'framework "{value}" is the {belongs_to} framework; the {task} task '
            f'needs "{expected}" or "custom".'
        )
    return value


def _chain_id(payload: dict[str, Any], name: str) -> str:
    value = text(payload, name, max_length=4)
    if not _CHAIN.fullmatch(value):
        raise ToolInputError(f"{name} must be a single-character chain ID.")
    return value


def _antigen_chains(payload: dict[str, Any]) -> list[str]:
    values = [
        value.strip()
        for value in text(payload, "antigen_chains", max_length=60).split(",")
        if value.strip()
    ]
    if not values:
        raise ToolInputError("antigen_chains must name at least one chain.")
    for value in values:
        if not _CHAIN.fullmatch(value):
            raise ToolInputError(
                f'antigen_chains entry "{value}" is not a single-character chain ID.'
            )
    if len(set(values)) != len(values):
        raise ToolInputError("antigen_chains names the same chain twice.")
    return values


def _hotspots(payload: dict[str, Any], antigen_chains: list[str]) -> list[str]:
    values = [
        value.strip()
        for value in text(payload, "hotspots", required=False, max_length=500).split(
            ","
        )
        if value.strip()
    ]
    for value in values:
        # Split off the chain by position rather than by pattern: a PDB chain
        # ID is one character and may itself be a digit, so "305" would
        # otherwise read as residue 05 on chain 3.
        if value[:1] not in antigen_chains or not _RESIDUE.fullmatch(value[1:]):
            raise ToolInputError(
                f'hotspots entry "{value}" must be one of the antigen chains '
                f'({", ".join(antigen_chains)}) followed by a residue number, '
                f"such as {antigen_chains[0]}305."
            )
    return values


def _regions(payload: dict[str, Any], task: str) -> list[tuple[str, str]]:
    """The CDR loops to design, as (form name, RFantibody code), in HLT order."""

    available = _TASK_LOOPS[task]
    names = [
        value.strip().lower()
        for value in text(payload, "regions", max_length=60).split(",")
        if value.strip()
    ]
    if not names:
        raise ToolInputError("regions must name at least one CDR loop.")
    chosen: list[str] = []
    for name in names:
        if name in chosen:
            raise ToolInputError(f'regions names "{name}" twice.')
        if name not in available:
            if name in _LIGHT_LOOPS:
                raise ToolInputError(
                    f'regions names "{name}", which a nanobody framework does not have.'
                )
            raise ToolInputError(f'Unsupported regions entry: "{name}".')
        chosen.append(name)
    order = list(available)
    chosen.sort(key=order.index)
    return [(name, available[name]) for name in chosen]


def _design_loops(payload: dict[str, Any], regions: list[tuple[str, str]]) -> str:
    """RFdiffusion's -l argument: one CODE:LENGTH entry per designed loop.

    An empty length -- "H1:" -- is how RFantibody says "design this loop but
    keep the framework's own length for it", which is what "auto" means here.
    """

    tokens = []
    for name, code in regions:
        field = f"{name}_length"
        raw = text(payload, field, required=False, max_length=20).strip().lower()
        value = raw or "auto"
        if value == "auto":
            tokens.append(f"{code}:")
            continue
        if not _LENGTH.fullmatch(value):
            raise ToolInputError(
                f'{field} must be "auto", a length such as 7, or a range such as 5-13.'
            )
        if "-" in value:
            start, end = (int(part) for part in value.split("-"))
            if start < 1 or start > end:
                raise ToolInputError(
                    f'{field} range "{value}" must run from at least 1 upwards.'
                )
        elif int(value) < 1:
            raise ToolInputError(f"{field} must be at least 1.")
        tokens.append(f"{code}:{value}")
    return ",".join(tokens)


def _residue_indices(payload: dict[str, Any], field: str) -> list[int]:
    """1-indexed absolute residue numbers from "26-32" or "26,27,28"."""

    raw = text(payload, field, required=False, max_length=200)
    if not raw:
        return []
    numbers: list[int] = []
    for token in raw.replace(" ", "").split(","):
        if not token:
            continue
        try:
            if "-" in token:
                first, last = token.split("-", 1)
                start, end = int(first), int(last)
            else:
                start = end = int(token)
        except ValueError as exc:
            raise ToolInputError(
                f'{field} entry "{token}" is not a residue number or range.'
            ) from exc
        if start < 1 or end < start or end - start > 100:
            raise ToolInputError(
                f'{field} entry "{token}" is not a plausible residue range.'
            )
        numbers.extend(range(start, end + 1))
    return sorted(set(numbers))


def _cdr_indices(payload: dict[str, Any], task: str) -> dict[str, list[int]]:
    if not boolean(payload, "select_cdr_indices", False):
        return {}
    indices = {}
    for name, code in _TASK_LOOPS[task].items():
        numbers = _residue_indices(payload, f"{name}_residues")
        if numbers:
            indices[code] = numbers
    if not indices:
        raise ToolInputError(
            "select_cdr_indices is set but no CDR residue numbers were given."
        )
    return indices


def _annotate_cdrs(framework: str, indices: dict[str, list[int]]) -> str:
    """Replace the HLT CDR annotations for the loops the request re-specified.

    The loops that were not given indices keep whatever the framework was
    labelled with, so this narrows one CDR without having to restate the rest.
    """

    def replaced(line: str) -> bool:
        if not line.startswith("REMARK PDBinfo-LABEL:"):
            return False
        parts = line.split()
        return bool(parts) and parts[-1] in indices

    kept = [line for line in framework.splitlines() if not replaced(line)]
    labels = [
        f"REMARK PDBinfo-LABEL: {number:4d} {code}"
        for code in sorted(indices)
        for number in indices[code]
    ]
    return "\n".join([*kept, *labels]) + "\n"


def _antigen_only(pdb: str, chains: list[str]) -> str:
    """The target PDB cut down to the chains the request named.

    Both remaining stages scale as O(N^2) in residue count, so anything the
    epitope does not need is dropped before the first one starts.
    """

    wanted = set(chains)
    lines: list[str] = []
    seen: set[str] = set()
    for line in pdb.splitlines():
        if line.startswith(("ATOM", "HETATM")):
            if len(line) < 22 or line[21] not in wanted:
                continue
            seen.add(line[21])
            lines.append(line)
        elif line.startswith("TER") and len(line) > 21 and line[21] in wanted:
            lines.append(line)
    missing = sorted(wanted - seen)
    if missing:
        raise ToolInputError(
            f'antigen_chains names {", ".join(missing)}, which the target PDB '
            "does not have."
        )
    return "\n".join([*lines, "END"]) + "\n"


def _sequence_weights(root: Path, use_abmpnn: bool) -> Path:
    if not use_abmpnn:
        return root / "weights" / "ProteinMPNN_v48_noise_0.2.pt"
    # ABMPNN_MODEL_PATH holds ProteinMPNN's weights directory, the shape the
    # AbMPNN adapter wants; RFantibody's sequence-design script takes a single
    # checkpoint file out of it instead.
    configured = os.getenv("ABMPNN_MODEL_PATH")
    if not configured:
        raise ToolUnavailable(
            "abmpnn_weights needs ABMPNN_MODEL_PATH set to the AbMPNN weights "
            "directory."
        )
    checkpoint = Path(configured) / "v_48_020.pt"
    if not checkpoint.is_file():
        raise ToolUnavailable(f"No AbMPNN checkpoint at {checkpoint}.")
    return checkpoint


def _epitope_distances(diffusion_log: str) -> list[dict[str, float]]:
    """Per design, how close the hotspots ended up to a designed CDR residue."""

    closest = [float(value) for value in _CLOSEST_DISTANCE.findall(diffusion_log)]
    mean = [float(value) for value in _MEAN_DISTANCE.findall(diffusion_log)]
    return [
        {"closest_angstroms": one, "mean_angstroms": other}
        for one, other in zip(closest, mean)
    ]


def _write_framework(
    payload: dict[str, Any],
    task: str,
    framework: str,
    root: Path,
    python: str,
    workdir: Path,
    environment: dict[str, str],
) -> Path:
    """The HLT-formatted scaffold the diffusion stage designs onto."""

    framework_path = workdir / "framework.pdb"
    if framework != "custom":
        bundled = root / "scripts" / "examples" / "example_inputs" / f"{framework}.pdb"
        if not bundled.is_file():
            raise ToolUnavailable(
                f"The {framework} framework is missing from {bundled.parent}."
            )
        framework_path.write_text(bundled.read_text(encoding="utf-8"), encoding="utf-8")
        return framework_path

    antibody_pdb = text(payload, "antibody_file", required=False, max_length=4_000_000)
    if not antibody_pdb:
        raise ToolInputError('antibody_file is required when framework is "custom".')
    converter = root / "scripts" / "util" / "chothia2HLT.py"
    if not converter.is_file():
        raise ToolUnavailable(f"{converter} is missing from the checkout.")

    source = workdir / "antibody.pdb"
    source.write_text(antibody_pdb + "\n", encoding="utf-8")
    command = [
        python,
        str(converter),
        str(source),
        "-H",
        _chain_id(payload, "heavy_chain"),
        "-o",
        str(framework_path),
    ]
    if task == "antibody":
        command += ["-L", _chain_id(payload, "light_chain")]
    run_command(command, cwd=root, env=environment, artifacts=[workdir])
    if not framework_path.is_file():
        raise ToolInputError(
            "The antibody PDB could not be converted to HLT format; check that it is "
            "Chothia-numbered and that the chain IDs are right."
        )
    return framework_path


def run(payload: dict[str, Any]) -> dict[str, Any]:
    name = safe_name(payload, default="rfantibody-demo")
    task = _task(payload)
    framework = _framework(payload, task)
    target_input = text(payload, "target_file", max_length=4_000_000)
    try:
        target_pdb = bio_tools.catalog_input_text("rfantibody", target_input)
    except ValueError as exc:
        raise ToolInputError(str(exc)) from exc
    antigen_chains = _antigen_chains(payload)
    target_pdb = _antigen_only(target_pdb, antigen_chains)
    hotspots = _hotspots(payload, antigen_chains)
    regions = _regions(payload, task)
    design_loops = _design_loops(payload, regions)
    cdr_indices = _cdr_indices(payload, task)
    report_distance = boolean(payload, "calculate_epitope_distance", True)
    num_designs = integer(payload, "num_designs", default=1, minimum=1, maximum=1_000)
    temperature = decimal(
        payload, "temperature", default=0.1, minimum=0.0001, maximum=1.0
    )
    seqs_per_struct = integer(
        payload, "seqs_per_struct", default=1, minimum=1, maximum=100
    )
    omit_aas = "".join(
        text(payload, "omit_AAs", required=False, max_length=21).upper().split()
    )
    if any(not character.isalpha() for character in omit_aas):
        raise ToolInputError("omit_AAs must contain only one-letter amino-acid codes.")
    augment_eps = decimal(
        payload, "augment_eps", default=0.05, minimum=0.0, maximum=1.0
    )
    num_connections = integer(
        payload, "num_connections", default=48, minimum=8, maximum=64
    )
    n_recycles = integer(payload, "n_recycles", default=3, minimum=1, maximum=20)
    hotspot_percentage = decimal(
        payload, "hotspot_percentage", default=0.1, minimum=0.0, maximum=1.0
    )
    save_trajectories = boolean(payload, "save_trajectories", False)

    root = _root()
    if not root.is_dir():
        raise ToolUnavailable("Configure RFANTIBODY_ROOT with the RFantibody checkout.")
    for filename in _STAGE_WEIGHTS:
        if not (root / "weights" / filename).is_file():
            raise ToolUnavailable(
                f"RFantibody weight {filename} is missing from {root / 'weights'}."
            )
    diffusion_runner = tool_script("rfantibody", "rfdiffusion", "RFANTIBODY_EXECUTABLE")
    prediction_runner = tool_script("rfantibody", "rf2", "RFANTIBODY_RF2_EXECUTABLE")
    python = tool_python("rfantibody", "RFANTIBODY_PYTHON")
    sequence_script = root / "scripts" / "proteinmpnn_interface_design.py"
    if not sequence_script.is_file():
        raise ToolUnavailable(f"{sequence_script} is missing from the checkout.")
    sequence_weights = _sequence_weights(
        root, boolean(payload, "abmpnn_weights", False)
    )
    environment = _stage_environment(python)

    with tempfile.TemporaryDirectory(prefix="bio-web-rfantibody-") as temporary:
        workdir = Path(temporary)
        target_path = workdir / "target.pdb"
        target_path.write_text(target_pdb, encoding="utf-8")

        framework_path = _write_framework(
            payload, task, framework, root, python, workdir, environment
        )
        if cdr_indices:
            framework_path.write_text(
                _annotate_cdrs(framework_path.read_text(encoding="utf-8"), cdr_indices),
                encoding="utf-8",
            )

        backbones = workdir / "1_backbones"
        sequences = workdir / "2_sequences"
        predictions = workdir / "3_predictions"
        for directory in (backbones, sequences, predictions):
            directory.mkdir()

        command = [
            diffusion_runner,
            "--target",
            str(target_path),
            "--framework",
            str(framework_path),
            "--output",
            str(backbones / name),
            "--num-designs",
            str(num_designs),
            "--design-loops",
            design_loops,
        ]
        if hotspots:
            command += ["--hotspots", ",".join(hotspots)]
        if not save_trajectories:
            command.append("--no-trajectory")
        diffusion = run_command(command, cwd=root, env=environment, artifacts=[workdir])

        run_command(
            [
                python,
                str(sequence_script),
                "-pdbdir",
                str(backbones),
                "-outpdbdir",
                str(sequences),
                "-loop_string",
                ",".join(code for _, code in regions),
                "-seqs_per_struct",
                str(seqs_per_struct),
                "-temperature",
                str(temperature),
                "-omit_AAs",
                omit_aas,
                "-augment_eps",
                str(augment_eps),
                "-num_connections",
                str(num_connections),
                "-checkpoint_path",
                str(sequence_weights),
            ],
            cwd=root,
            env=environment,
            artifacts=[workdir],
        )

        result = run_command(
            [
                prediction_runner,
                "--input-dir",
                str(sequences),
                "--output-dir",
                str(predictions),
                "--num-recycles",
                str(n_recycles),
                "--hotspot-show-prop",
                str(hotspot_percentage),
            ],
            cwd=root,
            env=environment,
            artifacts=[workdir],
        )
        generated = readable_files(predictions)

    response: dict[str, Any] = {
        "status": "completed",
        "job_name": name,
        "task": task,
        "framework": framework,
        "design_loops": design_loops,
        "hotspots": hotspots,
        "generated_files": generated,
        **result,
    }
    if report_distance:
        response["epitope_distances"] = _epitope_distances(diffusion["stdout"])
    return response


def check_status() -> ToolStatus:
    try:
        runner = tool_script("rfantibody", "rfdiffusion", "RFANTIBODY_EXECUTABLE")
        tool_script("rfantibody", "rf2", "RFANTIBODY_RF2_EXECUTABLE")
        tool_python("rfantibody", "RFANTIBODY_PYTHON")
    except ToolUnavailable as exc:
        return ToolStatus(CheckResult.NOT_INSTALLED, str(exc))
    root = _root()
    if not root.is_dir():
        return ToolStatus(CheckResult.NOT_INSTALLED, f"Missing {root}.")
    for filename in _STAGE_WEIGHTS:
        weights = root / "weights" / filename
        if not weights.is_file():
            return ToolStatus(CheckResult.NOT_INSTALLED, f"Missing {weights}.")
    status = probe_cli([runner], cwd=root)
    if status.result != CheckResult.PASS:
        return status
    return require_gpu_status(
        status, torch_device(environment_python("rfantibody")), "RFantibody"
    )
