"""GROMACS preparation, equilibration and production MD."""

from __future__ import annotations

import base64
import binascii
import io
import re
import tempfile
import zipfile
from pathlib import Path, PurePosixPath
from typing import Any

from . import (
    ToolInputError,
    ToolUnavailable,
    catalog_spec,
    executable,
    probe_command,
    readable_files,
    run_command,
    text,
    tool_fields,
)
from .field_processing import boolean, choice, decimal, integer
from .status_check import CheckResult, ToolStatus

SPEC = catalog_spec("gromacs", fields=tool_fields("gromacs"))
MAX_INPUT = 20_000_000
MAX_BUNDLE = 100_000_000
FORCE_FIELDS = {"amber99sb-ildn", "amber03", "amber99sb", "charmm27", "oplsaa"}


def relative_path(value: str) -> PurePosixPath:
    path = PurePosixPath(value)
    if (
        not value
        or "\\" in value
        or ":" in value
        or "\x00" in value
        or path.is_absolute()
        or any(p in {"..", "."} for p in value.split("/"))
    ):
        raise ToolInputError("Input filenames must be relative paths without '..'.")
    return path


def unpack_bundle(encoded: str, root: Path) -> None:
    """Extract a bounded ZIP of regular input files, retaining include directories."""
    if len(encoded) > MAX_BUNDLE * 4 // 3 + 4:
        raise ToolInputError("The input ZIP is too large (100 MB maximum).")
    try:
        raw = base64.b64decode(encoded, validate=True)
        with zipfile.ZipFile(io.BytesIO(raw)) as bundle:
            members = bundle.infolist()
            if len(members) > 1000 or sum(m.file_size for m in members) > MAX_BUNDLE:
                raise ToolInputError(
                    "The input ZIP exceeds 1000 files or 100 MB expanded."
                )
            seen = set()
            for member in members:
                name = relative_path(member.filename.rstrip("/"))
                mode = member.external_attr >> 16
                if mode & 0o170000 not in {0, 0o040000, 0o100000}:
                    raise ToolInputError(
                        "ZIP links and special files are not accepted."
                    )
                if member.is_dir():
                    continue
                if name in seen or member.flag_bits & 1:
                    raise ToolInputError(
                        "ZIP files must have unique names and no encryption."
                    )
                seen.add(name)
                target = root / name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(bundle.read(member))
    except (
        ValueError,
        binascii.Error,
        zipfile.BadZipFile,
        OSError,
        RuntimeError,
    ) as exc:
        raise ToolInputError(f"Invalid input ZIP: {exc}") from exc


def validate_includes(root: Path) -> None:
    pending = [
        p for p in root.rglob("*") if p.suffix.lower() in {".top", ".itp", ".mdp"}
    ]
    checked = set()
    while pending:
        path = pending.pop()
        if path in checked:
            continue
        checked.add(path)
        try:
            content = path.read_text(encoding="utf-8")
        except UnicodeError as exc:
            raise ToolInputError(f"{path.name} must be UTF-8 text.") from exc
        for line in content.splitlines():
            line = line.split(";", 1)[0].strip()
            if re.match(r"#\s*include\b", line):
                match = re.fullmatch(r'#\s*include\s+["<]([^">]+)[">]\s*', line)
                if not match:
                    raise ToolInputError(
                        "Topology includes must use literal relative filenames."
                    )
                name = relative_path(match[1])
                for included in (path.parent / name, root / name):
                    if included.is_file():
                        pending.append(included)
            if path.suffix.lower() == ".mdp" and re.match(
                r"include\s*=\s*\S", line, re.I
            ):
                raise ToolInputError(
                    "Put topology includes in the ZIP; MDP include search paths are not accepted."
                )


def structure_file(payload: dict[str, Any], root: Path) -> str:
    # MOL/SDF headers may begin with blank lines; their line positions matter.
    content = str(payload.get("pdb") or "")
    if not content.strip() or len(content) > MAX_INPUT:
        raise ToolInputError(
            "Molecule coordinates are required and must be at most 20 MB."
        )
    fmt = choice(
        payload,
        "structure_format",
        {"auto", "pdb", "gro", "mmcif", "mol2", "sdf"},
        "auto",
    )
    if fmt == "auto":
        if "@<TRIPOS>MOLECULE" in content:
            fmt = "mol2"
        elif "_atom_site." in content:
            fmt = "mmcif"
        elif "V2000" in content or "V3000" in content:
            fmt = "sdf"
        elif any(
            line.startswith(("ATOM  ", "HETATM")) for line in content.splitlines()
        ):
            fmt = "pdb"
        else:
            fmt = "gro"
    if fmt == "mmcif":
        from Bio.PDB import MMCIFParser, PDBIO

        try:
            structure = MMCIFParser(QUIET=True).get_structure(
                "input", io.StringIO(content)
            )
            writer = PDBIO()
            writer.set_structure(next(structure.get_models()))
            output = io.StringIO()
            writer.save(output)
            content, fmt = output.getvalue(), "pdb"
        except Exception as exc:
            raise ToolInputError(
                f"Cannot convert mmCIF to GROMACS PDB input: {exc}"
            ) from exc
    elif fmt in {"mol2", "sdf"}:
        from rdkit import Chem

        try:
            mol = (
                Chem.MolFromMol2Block(content, removeHs=False, sanitize=False)
                if fmt == "mol2"
                else Chem.MolFromMolBlock(content, removeHs=False, sanitize=False)
            )
            if (
                mol is None
                or not mol.GetNumConformers()
                or not mol.GetConformer().Is3D()
            ):
                raise ValueError("Supply a molecule with 3D coordinates.")
            if (
                fmt == "sdf"
                and len([s for s in content.split("$$$$") if s.strip()]) > 1
            ):
                raise ValueError(
                    "Supply one SDF molecule, or a combined GRO/PDB system."
                )
            # Preserve atom order and hydrogens for the supplied topology.
            for atom in mol.GetAtoms():
                if atom.HasProp("_TriposAtomName"):
                    info = Chem.AtomPDBResidueInfo()
                    info.SetName(atom.GetProp("_TriposAtomName").rjust(4))
                    info.SetResidueName("MOL")
                    info.SetResidueNumber(1)
                    info.SetIsHeteroAtom(True)
                    atom.SetMonomerInfo(info)
            content, fmt = Chem.MolToPDBBlock(mol), "pdb"
        except Exception as exc:
            raise ToolInputError(f"Cannot read molecule: {exc}") from exc
        if not str(payload.get("topology", "")).strip():
            raise ToolInputError(
                "SDF/Mol2 input requires a matching GROMACS topology and any ITP includes. Atom order and hydrogens must match."
            )
    filename = f"input.{fmt}"
    (root / filename).write_text(content + "\n", encoding="utf-8")
    return filename


def parameters(payload: dict[str, Any]) -> dict[str, Any]:
    p = {
        "integrator": choice(payload, "integrator", {"md", "md-vv", "sd"}, "md"),
        "nsteps": integer(
            payload, "nsteps", default=5000, minimum=1, maximum=1_000_000_000
        ),
        "dt": decimal(payload, "dt", default=0.002, minimum=0.000001, maximum=0.02),
        "temperature": decimal(
            payload, "temperature", default=300, minimum=1, maximum=2000
        ),
        "pressure": decimal(
            payload, "pressure", default=1, minimum=0.01, maximum=10000
        ),
        "ensemble": choice(payload, "ensemble", {"nvt", "npt", "nve"}, "nvt"),
        "tcoupl": choice(payload, "tcoupl", {"v-rescale", "nose-hoover"}, "v-rescale"),
        "tau_t": decimal(payload, "tau_t", default=0.1, minimum=0.001, maximum=100),
        "tau_p": decimal(payload, "tau_p", default=5, minimum=0.1, maximum=100),
        "compressibility": decimal(
            payload, "compressibility", default=4.5e-5, minimum=1e-10, maximum=1
        ),
        "cutoff": decimal(payload, "cutoff", default=1, minimum=0.1, maximum=5),
        "constraints": choice(
            payload, "constraints", {"none", "h-bonds", "all-bonds"}, "h-bonds"
        ),
        "coulombtype": choice(
            payload, "coulombtype", {"PME", "Cut-off", "Reaction-Field"}, "PME"
        ),
        "epsilon_rf": decimal(
            payload, "epsilon_rf", default=78.3, minimum=0, maximum=1000
        ),
        "output_stride": integer(
            payload, "output_stride", default=100, minimum=1, maximum=1_000_000
        ),
        "seed": integer(payload, "seed", default=-1, minimum=-1, maximum=2147483647),
    }
    if p["ensemble"] == "nve" and p["integrator"] == "sd":
        raise ToolInputError(
            "Stochastic dynamics has a thermostat; choose md or md-vv for NVE."
        )
    return p


def mdp_text(
    p: dict[str, Any],
    *,
    nsteps: int | None = None,
    ensemble: str | None = None,
    continuation: bool = False,
) -> str:
    ensemble = ensemble or p["ensemble"]
    thermostat = ensemble != "nve"
    values = {
        "integrator": p["integrator"],
        "dt": p["dt"],
        "nsteps": nsteps or p["nsteps"],
        "cutoff-scheme": "Verlet",
        "nstlist": 20,
        "pbc": "xyz",
        "coulombtype": p["coulombtype"],
        "rcoulomb": p["cutoff"],
        "vdwtype": "Cut-off",
        "rvdw": p["cutoff"],
        "DispCorr": "EnerPres",
        "constraints": p["constraints"],
        "constraint-algorithm": "lincs",
        "continuation": "yes" if continuation else "no",
        "tcoupl": p["tcoupl"] if thermostat and p["integrator"] != "sd" else "no",
        "pcoupl": "C-rescale" if ensemble == "npt" else "no",
        "gen-vel": "no" if continuation else "yes",
        "nstxout": p["output_stride"],
        "nstvout": 0,
        "nstfout": 0,
        "nstxout-compressed": p["output_stride"],
        "nstenergy": p["output_stride"],
        "nstlog": p["output_stride"],
    }
    if thermostat:
        values.update(
            {"tc-grps": "System", "tau-t": p["tau_t"], "ref-t": p["temperature"]}
        )
    if not continuation:
        values.update({"gen-temp": p["temperature"], "gen-seed": p["seed"]})
    if p["integrator"] == "sd":
        values["ld-seed"] = p["seed"]
    if p["coulombtype"] == "Reaction-Field":
        values["epsilon-rf"] = p["epsilon_rf"]
    if ensemble == "npt":
        values.update(
            {
                "pcoupltype": "isotropic",
                "tau-p": p["tau_p"],
                "ref-p": p["pressure"],
                "compressibility": p["compressibility"],
            }
        )
    return "".join(f"{key} = {value}\n" for key, value in values.items())


def run(payload: dict[str, Any]) -> dict[str, Any]:
    mode = choice(payload, "input_mode", {"parameters", "text", "upload"}, "parameters")
    device = choice(payload, "device", {"auto", "cpu", "gpu"}, "auto")
    threads = integer(payload, "threads", default=0, minimum=0, maximum=256)
    gmx = executable("GROMACS_EXECUTABLE", "gmx", "gmx_mpi")
    logs: list[dict[str, Any]] = []

    with tempfile.TemporaryDirectory(prefix="bio-tools-gromacs-") as temporary:
        root = Path(temporary)
        work = root / "md_out"
        work.mkdir()

        def command(
            *args: str, stdin: str | None = None, artifacts: list[Path] | None = None
        ):
            result = run_command(
                [gmx, *args],
                cwd=work,
                stdin=stdin,
                env={"GMX_MAXBACKUP": "-1"},
                artifacts=artifacts,
            )
            logs.append(result)

        if mode == "upload":
            inputs = work / "inputs"
            inputs.mkdir()
            unpack_bundle(
                text(payload, "input_bundle", max_length=MAX_BUNDLE * 4 // 3 + 4),
                inputs,
            )
            names = {}
            for key, default in (
                ("coordinates_path", "conf.gro"),
                ("topology_path", "topol.top"),
                ("mdp_path", "md.mdp"),
            ):
                name = str(payload.get(key) or default).strip()
                path = inputs / relative_path(name)
                if not path.is_file():
                    raise ToolInputError(f"The ZIP is missing {name}.")
                names[key] = f"inputs/{name}"
            coordinates, topology, mdp = (
                names["coordinates_path"],
                names["topology_path"],
                names["mdp_path"],
            )
            if Path(coordinates).suffix.lower() not in {".gro", ".pdb"}:
                raise ToolInputError("Raw coordinates must be GRO or PDB.")
            if (
                Path(topology).suffix.lower() != ".top"
                or Path(mdp).suffix.lower() != ".mdp"
            ):
                raise ToolInputError(
                    "The topology and MDP paths must end in .top and .mdp."
                )
        else:
            includes = text(
                payload,
                "supporting_files",
                required=False,
                max_length=MAX_BUNDLE * 4 // 3 + 4,
            )
            if includes:
                unpack_bundle(includes, work)
                for stage in ("em", "nvt", "npt", "md"):
                    for suffix in ("gro", "xtc", "trr", "edr", "log", "tpr", "cpt"):
                        if (work / f"{stage}.{suffix}").exists():
                            raise ToolInputError(
                                "The topology ZIP must not contain simulation output files."
                            )
            coordinates = structure_file(payload, work)
            topology, mdp = "topol.top", "md.mdp"
            top = text(
                payload, "topology", required=mode == "text", max_length=MAX_INPUT
            )
            if top:
                (work / topology).write_text(top + "\n", encoding="utf-8")
            if mode == "text":
                (work / mdp).write_text(
                    text(payload, "mdp", max_length=MAX_INPUT) + "\n", encoding="utf-8"
                )
            else:
                p = parameters(payload)
                force_field = choice(
                    payload, "force_field", FORCE_FIELDS, "amber99sb-ildn"
                )
                water = choice(
                    payload, "water_model", {"tip3p", "spce", "none"}, "tip3p"
                )
                box = choice(
                    payload,
                    "box_type",
                    {"dodecahedron", "cubic", "octahedron"},
                    "dodecahedron",
                )
                distance = decimal(
                    payload, "box_distance", default=1.2, minimum=0.1, maximum=10
                )
                validate_includes(work)
                if not top:
                    command(
                        "pdb2gmx",
                        "-f",
                        coordinates,
                        "-o",
                        "processed.gro",
                        "-p",
                        topology,
                        "-ff",
                        force_field,
                        "-water",
                        water,
                        "-ignh",
                    )
                    coordinates = "processed.gro"
                command(
                    "editconf",
                    "-f",
                    coordinates,
                    "-o",
                    "boxed.gro",
                    "-c",
                    "-d",
                    str(distance),
                    "-bt",
                    box,
                )
                coordinates = "boxed.gro"
                if boolean(payload, "solvate", True) and water != "none":
                    command(
                        "solvate",
                        "-cp",
                        coordinates,
                        "-cs",
                        "spc216.gro",
                        "-o",
                        "solvated.gro",
                        "-p",
                        topology,
                    )
                    coordinates = "solvated.gro"
                em_steps = integer(
                    payload, "em_steps", default=5000, minimum=1, maximum=1_000_000
                )
                emtol = decimal(
                    payload, "emtol", default=1000, minimum=0.01, maximum=100000
                )
                (work / "em.mdp").write_text(
                    f"integrator = steep\nnsteps = {em_steps}\nemtol = {emtol}\nemstep = 0.01\n"
                    f"cutoff-scheme = Verlet\ncoulombtype = {p['coulombtype']}\n"
                    f"rcoulomb = {p['cutoff']}\nrvdw = {p['cutoff']}\npbc = xyz\n",
                    encoding="utf-8",
                )
                salt = decimal(
                    payload, "salt_concentration", default=0.15, minimum=0, maximum=5
                )
                neutralize = boolean(payload, "neutralize", True)
                if (neutralize or salt) and coordinates == "solvated.gro":
                    command(
                        "grompp",
                        "-f",
                        "em.mdp",
                        "-c",
                        coordinates,
                        "-p",
                        topology,
                        "-o",
                        "ions.tpr",
                        "-po",
                        "ions-out.mdp",
                    )
                    args = [
                        "genion",
                        "-s",
                        "ions.tpr",
                        "-o",
                        "ionized.gro",
                        "-p",
                        topology,
                        "-pname",
                        "NA",
                        "-nname",
                        "CL",
                        "-conc",
                        str(salt),
                    ]
                    if neutralize:
                        args.append("-neutral")
                    command(*args, stdin="SOL\n")
                    coordinates = "ionized.gro"
                (work / mdp).write_text(mdp_text(p), encoding="utf-8")

        validate_includes(work)
        index = text(payload, "index", required=False, max_length=MAX_INPUT)
        if index and mode != "upload":
            (work / "index.ndx").write_text(index + "\n", encoding="utf-8")
        ndx_name = "inputs/index.ndx" if mode == "upload" else "index.ndx"
        ndx = ndx_name if (work / ndx_name).is_file() else None
        snapshot_inputs = True

        def simulate(
            prefix: str,
            mdp_name: str,
            start: str,
            checkpoint: str | None = None,
            minimization: bool = False,
        ) -> str:
            nonlocal snapshot_inputs
            args = [
                "grompp",
                "-f",
                mdp_name,
                "-c",
                start,
                "-r",
                start,
                "-p",
                topology,
                "-o",
                f"{prefix}.tpr",
                "-po",
                f"{prefix}-out.mdp",
                "-pp",
                f"{prefix}-processed.top",
            ]
            if ndx:
                args += ["-n", ndx]
            if checkpoint:
                args += ["-t", checkpoint]
            artifacts = (
                [work]
                if snapshot_inputs
                else [
                    work / mdp_name,
                    work / topology,
                    work / start,
                    work / f"{prefix}.tpr",
                    work / f"{prefix}-out.mdp",
                    work / f"{prefix}-processed.top",
                ]
            )
            command(*args, artifacts=artifacts)
            snapshot_inputs = False
            args = [
                "mdrun",
                "-deffnm",
                prefix,
                "-nb",
                "cpu" if minimization else device,
            ]
            if "mpi" not in Path(gmx).name:
                args += ["-ntmpi", "1"]
            if threads:
                args += ["-ntomp", str(threads)]
            command(
                *args,
                artifacts=[
                    work / f"{prefix}.{ext}"
                    for ext in ("gro", "xtc", "trr", "edr", "log", "cpt")
                ],
            )
            return f"{prefix}.gro"

        if mode == "parameters":
            if boolean(payload, "minimize", True):
                coordinates = simulate("em", "em.mdp", coordinates, minimization=True)
            checkpoint = None
            for stage in ("nvt", "npt"):
                count = integer(
                    payload, f"{stage}_steps", default=0, minimum=0, maximum=100_000_000
                )
                if count:
                    (work / f"{stage}.mdp").write_text(
                        mdp_text(
                            p,
                            nsteps=count,
                            ensemble=stage,
                            continuation=checkpoint is not None,
                        ),
                        encoding="utf-8",
                    )
                    coordinates = simulate(
                        stage, f"{stage}.mdp", coordinates, checkpoint
                    )
                    checkpoint = f"{stage}.cpt"
            (work / mdp).write_text(
                mdp_text(p, continuation=checkpoint is not None), encoding="utf-8"
            )
        else:
            checkpoint_name = "inputs/state.cpt" if mode == "upload" else "state.cpt"
            checkpoint = checkpoint_name if (work / checkpoint_name).is_file() else None
        simulate("md", mdp, coordinates, checkpoint)
        generated = readable_files(work, maximum_files=1000)

    return {
        "status": "completed",
        "steps": logs,
        "generated_files": generated,
        "trajectory": {
            "structure": "md.gro",
            "coordinates": [n for n in ("md.xtc", "md.trr") if n in generated],
        },
    }


def check_status() -> ToolStatus:
    try:
        gmx = executable("GROMACS_EXECUTABLE", "gmx", "gmx_mpi")
    except ToolUnavailable as exc:
        return ToolStatus(CheckResult.NOT_INSTALLED, str(exc))
    code, output = probe_command([gmx, "--version"])
    if code is None:
        return ToolStatus(CheckResult.NOT_INSTALLED, f"Could not run gmx: {output}")
    if code != 0:
        return ToolStatus(CheckResult.ERROR, output or f"gmx --version exited {code}.")
    gpu = re.search(r"GPU support:\s*(\S+)", output)
    device = (
        "GPU"
        if gpu and gpu.group(1).lower() not in {"disabled", "no", "none"}
        else "CPU"
    )
    version = next(
        (line.strip() for line in output.splitlines() if "GROMACS version" in line),
        "GROMACS",
    )
    return ToolStatus(CheckResult.PASS, version[:200], device=device)
