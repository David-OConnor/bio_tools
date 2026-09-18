"""NCBI IgBLAST: germline V(D)J annotation of antibody and TCR sequences.

Standalone executables, installed whole from NCBI's own distribution:
https://ftp.ncbi.nih.gov/blast/executables/igblast/release/LATEST/ . The
Ubuntu `igblast` package ships no igblastn/igblastp binaries, so `apt` is not
an option; setup_system.sh unpacks the tarball into process_executables/igblast
and downloads NCBI's prebuilt germline databases beside it.

Three directories of the distribution matter, and IgBLAST resolves two of them
through IGDATA rather than relative to the binary, so every invocation sets it:

  internal_data/   the per-organism annotation that places the FWR and CDR
                   boundaries. Keyed by `-organism`, which is why an organism
                   that disagrees with the germline databases is refused here
                   rather than quietly annotated at the wrong offsets.
  optional_file/   `<organism>_gl.aux`, the J-gene coding frames that let
                   IgBLAST find the end of CDR3 and FWR4, and `mouse_D.frame`.
                   Both are resolved automatically from the organism.
  germline_db/     the BLAST databases themselves, discovered on this host
                   (override the directory with IGBLAST_GERMLINE_ROOT).

What a run leaves, and what the results page offers first, is the AIRR
Rearrangement TSV `-outfmt 19` writes: one row per query, with the germline
calls, the junction and every region, in the schema the repertoire tooling
downstream of IgBLAST reads. The readable report is a second pass, because
IgBLAST writes one format per invocation.
"""

from __future__ import annotations

import os
import re
import tempfile
from pathlib import Path
from typing import Any

from . import (
    AMINO_ACIDS,
    DNA_BASES,
    PROCESS_EXECUTABLES,
    ToolInputError,
    ToolUnavailable,
    catalog_spec,
    executable,
    readable_files,
    run_command,
    text,
    tool_fields,
)
from .field_processing import boolean, choice, document_input, integer, safe_name
from .status_check import CheckResult, ToolStatus, probe_command

# `-organism`: the internal annotation data NCBI ships, one directory each
# under internal_data/. TCR is supported for human and mouse alone.
ORGANISMS = ("human", "mouse", "rat", "rabbit", "rhesus_monkey")
TCR_ORGANISMS = frozenset({"human", "mouse"})

# BLAST writes one header file per database, and its extension says whether the
# database holds nucleotide or protein sequences, which has to match the binary.
HEADER_SUFFIX = {"nucleotide": ".nhr", "protein": ".phr"}

# Databases are conventionally named after the segment they hold, as either
# airr_c_human_ig.V (NCBI's AIRR-C sets) or mouse_gl_V (the older sets).
SEGMENT_PATTERN = re.compile(r"[._-]([VDJ])$", re.IGNORECASE)

# Which prebuilt database NCBI recommends for each organism, as the setup guide
# pairs them: https://ncbi.github.io/igblast/cook/How-to-set-up.html . A
# segment an organism has no prebuilt database for is absent, and "Automatic"
# then leaves the option unset (D, C) or refuses the run (V, J).
#
# NCBI's AIRR-C mouse archive holds per-strain V sets only, so a mouse run
# pairs whichever V database is chosen with the NCBI mouse D and J sets.
RECOMMENDED: dict[str, dict[str, str]] = {
    "human": {
        "V": "airr_c_human_ig.V",
        "D": "airr_c_human_igh.D",
        "J": "airr_c_human_ig.J",
        "C": "ncbi_human_c_genes",
    },
    "mouse": {"V": "mouse_gl_V", "D": "mouse_gl_D", "J": "mouse_gl_J"},
    "rhesus_monkey": {"V": "rhesus_monkey_V", "J": "rhesus_monkey_J"},
}

# Which organism a prebuilt database describes, so a V set and an `-organism`
# that disagree are caught before IgBLAST annotates at the wrong offsets. A
# database an operator built themselves is not in here and is not second
# guessed; only a name this project installed is checked.
PREBUILT_ORGANISM = {
    "ncbi_human_c_genes": "human",
    "mouse_gl_V": "mouse",
    "mouse_gl_D": "mouse",
    "mouse_gl_J": "mouse",
    "rhesus_monkey_V": "rhesus_monkey",
    "rhesus_monkey_J": "rhesus_monkey",
}

# How a database name is described in the picker, by the archive it came from.
COLLECTIONS = (
    ("airr_c_human_", "AIRR-C human"),
    ("airr_c_", "AIRR-C mouse strain"),
    ("mouse_gl_", "NCBI mouse"),
    ("rhesus_monkey_", "NCBI rhesus monkey"),
    ("ncbi_human_c_genes", "NCBI human constant region"),
)

AUTOMATIC = "auto"
NO_DATABASE = ""

# The readable report formats igblastn and igblastp offer; `-outfmt 19` is the
# AIRR table, which this adapter writes as a pass of its own.
REPORT_FORMATS = {"3", "4", "7"}
AIRR_FORMAT = "19"

# Query FASTA limits. IgBLAST itself has none; these keep one submission inside
# the serialized run lane and inside what a results panel can render.
MAX_QUERY_CHARACTERS = 400_000
MAX_RECORDS = 5_000
# IUPAC nucleotide letters, which is what a sequencing read of a rearrangement
# carries. `sequence()` is not used because FASTA headers have to survive.
NUCLEOTIDES = DNA_BASES | {"-", "."}
# The shared set plus selenocysteine, pyrrolysine, the stop a translated read
# carries, and the gap characters an aligned sequence arrives with.
RESIDUES = AMINO_ACIDS | set("UO*-.")


def germline_root() -> Path:
    configured = os.getenv("IGBLAST_GERMLINE_ROOT")
    if configured:
        return Path(configured).expanduser()
    return PROCESS_EXECUTABLES / "igblast" / "germline_db"


def available_databases(sequence_type: str = "nucleotide") -> list[str]:
    """Names of the BLAST databases installed under the germline root."""

    suffix = HEADER_SUFFIX[sequence_type]
    root = germline_root()
    if not root.is_dir():
        return []

    names: set[str] = set()
    for path in root.rglob(f"*{suffix}"):
        stem = path.name[: -len(suffix)]
        # Databases large enough to be split carry a volume number, as in
        # human_gl_V.00.nhr; every volume maps back to one database name.
        stem = re.sub(r"\.\d{2,}$", "", stem)
        names.add((path.parent / stem).relative_to(root).as_posix())
    return sorted(names)


def databases_for(segment: str, sequence_type: str = "nucleotide") -> list[str]:
    """Installed databases holding `segment`, or all of them for custom names.

    The constant-region segment is asked for as "C": no naming convention
    marks one, so it is everything the V, D and J conventions do not claim.
    """

    names = available_databases(sequence_type)
    if segment == "C":
        return [name for name in names if not SEGMENT_PATTERN.search(name)] or names
    matching = [
        name
        for name in names
        if (match := SEGMENT_PATTERN.search(name)) and match.group(1).upper() == segment
    ]
    return matching or names


def _collection(name: str) -> str:
    """Which NCBI archive a database name came from, for its picker label."""

    stem = name.rsplit("/", 1)[-1]
    for prefix, label in COLLECTIONS:
        if stem.startswith(prefix):
            organism = PREBUILT_ORGANISM.get(stem)
            return f"{label} · {organism}" if organism else label
    return "installed here"


def _database_options(segment: str, *, optional: bool) -> list[tuple[str, str]]:
    """The picker for one segment, from the databases discovered on this host.

    "Automatic" is offered whether or not anything is discoverable here, and is
    always the first option: on a split deployment the form is rendered by the
    web node while the run happens on the compute node, which is the host that
    holds the germline databases. Offering only "No germline databases
    installed" made the web node post an empty selection that the compute node
    -- which does have the databases -- then refused as a missing V database.
    """

    options = [(AUTOMATIC, "Automatic (recommended set for the organism)")]
    if optional:
        options.append((NO_DATABASE, "None"))
    options.extend(
        (name, f"{name} ({_collection(name)})") for name in databases_for(segment)
    )
    return options


def _dynamic_field_options() -> dict[str, list[tuple[str, str]]]:
    return {
        "germline_db_v": _database_options("V", optional=False),
        "germline_db_d": _database_options("D", optional=True),
        "germline_db_j": _database_options("J", optional=True),
        "c_region_db": _database_options("C", optional=True),
    }


SPEC = catalog_spec(
    "igblast",
    fields=tool_fields("igblast", dynamic_options=_dynamic_field_options()),
    refresh_fields=lambda: tool_fields(
        "igblast", dynamic_options=_dynamic_field_options()
    ),
)


def _install_root(binary_path: str) -> Path | None:
    """The unpacked IgBLAST directory holding internal_data and optional_file."""

    configured = os.getenv("IGDATA")
    if configured and (Path(configured) / "internal_data").is_dir():
        return Path(configured)
    # The tarball layout is <root>/bin/igblastn alongside <root>/internal_data.
    root = Path(binary_path).resolve().parent.parent
    return root if (root / "internal_data").is_dir() else None


def parse_query(raw: str, sequence_type: str) -> list[tuple[str, str]]:
    """The query FASTA as records, validated but written back out unchanged.

    A bare sequence is given a header, because `-outfmt 19` keys every row on
    `sequence_id` and an unnamed query is one a reader cannot match back up.
    """

    lines = [line.strip() for line in raw.splitlines() if line.strip()]
    if not lines:
        raise ToolInputError("Provide a query sequence or FASTA records.")
    if not lines[0].startswith(">") and any(line.startswith(">") for line in lines):
        raise ToolInputError(
            "FASTA input must start with a header; do not mix bare sequences and "
            "FASTA records."
        )

    records: list[tuple[str, list[str]]] = []
    for line in lines:
        if line.startswith(">"):
            header = line[1:].strip()
            if not header or len(header) > 500:
                raise ToolInputError("FASTA headers must contain 1-500 characters.")
            records.append((header, []))
        else:
            if not records:
                records.append(("query_1", []))
            records[-1][1].append(re.sub(r"\s+", "", line).upper())

    if len(records) > MAX_RECORDS:
        raise ToolInputError(
            f"At most {MAX_RECORDS:,} query records are accepted per run."
        )

    alphabet = NUCLEOTIDES if sequence_type == "nucleotide" else RESIDUES
    kind = "nucleotide" if sequence_type == "nucleotide" else "amino-acid"
    seen: set[str] = set()
    parsed: list[tuple[str, str]] = []
    for header, chunks in records:
        # BLAST identifies a record by the first whitespace-delimited word of
        # its header, which is what has to be unique for the AIRR table.
        identifier = header.split()[0]
        if identifier in seen:
            raise ToolInputError(
                f'Query identifier "{identifier}" appears more than once; '
                "FASTA headers must start with a unique name."
            )
        seen.add(identifier)
        sequence = "".join(chunks)
        if not sequence:
            raise ToolInputError(f'Query "{identifier}" has no sequence.')
        invalid = set(sequence) - alphabet
        if invalid:
            raise ToolInputError(
                f'Query "{identifier}" contains characters that are not '
                f"{kind} letters: {''.join(sorted(invalid))}."
            )
        parsed.append((header, sequence))
    return parsed


def _from_boxes(payload: dict[str, Any]) -> str:
    """There is no parameter mode: IgBLAST's input really is a FASTA file."""

    raise ToolInputError(
        "Provide a query sequence or FASTA: input_mode must be text or upload."
    )


def _resolve_database(
    selected: str,
    *,
    segment: str,
    sequence_type: str,
    organism: str,
    required: bool,
) -> str | None:
    """One `-germline_db_*` argument: the chosen database, or none.

    "auto" resolves to NCBI's recommended prebuilt set for the organism; an
    explicit name is checked against what is installed, and against the
    organism where this project knows which one the database describes.
    """

    installed = databases_for(segment, sequence_type)
    if not installed:
        raise ToolUnavailable(
            f"No germline {segment} database is installed under {germline_root()}. "
            "Run setup_system.sh to download NCBI's prebuilt sets, or set "
            "IGBLAST_GERMLINE_ROOT to a directory holding your own."
        )

    if selected == AUTOMATIC:
        recommended = RECOMMENDED.get(organism, {}).get(segment)
        if recommended is None or recommended not in installed:
            if not required:
                return None
            raise ToolUnavailable(
                f"No prebuilt germline {segment} database is available for "
                f"{organism}. NCBI distributes prebuilt sets for human, mouse and "
                "rhesus monkey only; for any other organism, build the database "
                "from IMGT with bin/edit_imgt_file.pl and bin/makeblastdb and "
                "choose it explicitly. See "
                "https://ncbi.github.io/igblast/cook/How-to-set-up.html"
            )
        selected = recommended

    if not selected:
        if required:
            raise ToolInputError(
                f"A germline {segment} database is required. "
                f"Installed: {', '.join(installed)}."
            )
        return None

    if selected not in installed:
        raise ToolInputError(
            f"Unsupported germline {segment} database: {selected}. "
            f"Installed: {', '.join(installed)}."
        )

    described = PREBUILT_ORGANISM.get(selected.rsplit("/", 1)[-1])
    if described is not None and described != organism:
        raise ToolInputError(
            f'The germline {segment} database "{selected}" holds {described} genes, '
            f"but the organism is {organism}. IgBLAST annotates the framework and "
            "CDR boundaries from the organism's own internal data, so the two have "
            "to agree."
        )
    return str((germline_root() / selected).resolve())


def _optional_number(payload: dict[str, Any], name: str, label: str) -> str | None:
    """A BLAST option left at the program's own default when it is empty."""

    raw = text(payload, name, required=False, max_length=32)
    if not raw:
        return None
    try:
        float(raw)
    except ValueError as exc:
        raise ToolInputError(f"{label} must be a number, or empty.") from exc
    return raw


def _auxiliary_data(install_root: Path, organism: str) -> Path | None:
    """`<organism>_gl.aux`: the J coding frames that annotate CDR3's end."""

    configured = os.getenv("IGBLAST_AUXILIARY_DATA")
    candidate = (
        Path(configured)
        if configured
        else install_root / "optional_file" / f"{organism}_gl.aux"
    )
    return candidate if candidate.is_file() else None


def _d_frame_data(install_root: Path, organism: str) -> Path | None:
    """`<organism>_D.frame`, which the distribution ships for mouse alone."""

    candidate = install_root / "optional_file" / f"{organism}_D.frame"
    return candidate if candidate.is_file() else None


def prepare(payload: dict[str, Any]) -> dict[str, Any]:
    """Everything one run needs, validated, before any process is started."""

    payload = document_input(
        payload,
        "input_fasta",
        from_boxes=_from_boxes,
        max_length=MAX_QUERY_CHARACTERS,
    )
    sequence_type = choice(
        payload, "sequence_type", {"nucleotide", "protein"}, "nucleotide"
    )
    nucleotide = sequence_type == "nucleotide"
    records = parse_query(
        text(payload, "input_fasta", max_length=MAX_QUERY_CHARACTERS), sequence_type
    )

    organism = choice(payload, "organism", ORGANISMS, "human")
    ig_seqtype = choice(payload, "ig_seqtype", {"Ig", "TCR"}, "Ig")
    if ig_seqtype == "TCR" and organism not in TCR_ORGANISMS:
        raise ToolInputError(
            f"IgBLAST supports TCR annotation for {' and '.join(sorted(TCR_ORGANISMS))} "
            f"only, not {organism}."
        )

    d_penalty = integer(payload, "d_penalty", default=-2, minimum=-4, maximum=-1)
    j_penalty = integer(payload, "j_penalty", default=-2, minimum=-3, maximum=-1)
    allow_vdj_overlap = boolean(payload, "allow_vdj_overlap")
    if allow_vdj_overlap and (d_penalty, j_penalty) != (-4, -3):
        raise ToolInputError(
            "allow_vdj_overlap is active only when d_penalty is -4 and j_penalty "
            "is -3; IgBLAST would otherwise ignore it."
        )

    report_format = str(payload.get("report_format", "3")).strip()
    if report_format and report_format not in REPORT_FORMATS:
        raise ToolInputError(
            f"Unsupported report_format: {report_format}. "
            f"IgBLAST offers {', '.join(sorted(REPORT_FORMATS))}, or none."
        )
    write_airr = boolean(payload, "write_airr", True) and nucleotide
    if not write_airr and not report_format:
        raise ToolInputError(
            "This run would produce no output: choose a readable report format, "
            "or write the AIRR table."
            if nucleotide
            else "A protein query has no AIRR table, so it needs a report format."
        )

    num_clonotype = (
        integer(payload, "num_clonotype", default=100, minimum=0, maximum=100_000)
        if nucleotide
        else 0
    )

    return {
        "name": safe_name(payload, default="igblast-demo"),
        "sequence_type": sequence_type,
        "records": records,
        "organism": organism,
        "ig_seqtype": ig_seqtype,
        "databases": {
            "V": _resolve_database(
                str(payload.get("germline_db_v", AUTOMATIC)).strip(),
                segment="V",
                sequence_type=sequence_type,
                organism=organism,
                required=True,
            ),
            # igblastp aligns against V genes only: D, J and C apply to a
            # rearranged nucleotide query, and igblastp rejects the options.
            "D": _resolve_database(
                str(payload.get("germline_db_d", AUTOMATIC)).strip(),
                segment="D",
                sequence_type=sequence_type,
                organism=organism,
                required=False,
            )
            if nucleotide
            else None,
            "J": _resolve_database(
                str(payload.get("germline_db_j", AUTOMATIC)).strip(),
                segment="J",
                sequence_type=sequence_type,
                organism=organism,
                required=False,
            )
            if nucleotide
            else None,
            "C": _resolve_database(
                str(payload.get("c_region_db", AUTOMATIC)).strip(),
                segment="C",
                sequence_type=sequence_type,
                organism=organism,
                required=False,
            )
            if nucleotide
            else None,
        },
        "domain_system": choice(payload, "domain_system", {"imgt", "kabat"}, "imgt"),
        "show_translation": boolean(payload, "show_translation", True) and nucleotide,
        "extend_align5end": boolean(payload, "extend_align5end"),
        "extend_align3end": boolean(payload, "extend_align3end"),
        "min_v_length": integer(payload, "min_v_length", default=9, minimum=9, maximum=1000),
        "min_j_length": integer(payload, "min_j_length", default=0, minimum=0, maximum=1000),
        "min_d_match": integer(payload, "min_d_match", default=5, minimum=5, maximum=100),
        "v_penalty": integer(payload, "v_penalty", default=-1, minimum=-3, maximum=-1),
        "d_penalty": d_penalty,
        "j_penalty": j_penalty,
        "allow_vdj_overlap": allow_vdj_overlap,
        "write_airr": write_airr,
        "report_format": report_format,
        "num_alignments": {
            "V": integer(payload, "num_alignments_v", default=3, minimum=0, maximum=100),
            "D": integer(payload, "num_alignments_d", default=3, minimum=0, maximum=100),
            "J": integer(payload, "num_alignments_j", default=3, minimum=0, maximum=100),
            "C": integer(payload, "num_alignments_c", default=2, minimum=0, maximum=100),
        },
        "num_clonotype": num_clonotype,
        "evalue": _optional_number(payload, "evalue", "evalue"),
        "word_size": _optional_number(payload, "word_size", "word_size"),
        "gapopen": _optional_number(payload, "gapopen", "gapopen"),
        "gapextend": _optional_number(payload, "gapextend", "gapextend"),
        "strand": choice(payload, "strand", {"both", "plus", "minus"}, "both"),
        "num_threads": integer(payload, "num_threads", default=4, minimum=1, maximum=32),
    }


def _command(
    request: dict[str, Any],
    binary: str,
    install_root: Path,
    *,
    outfmt: str,
    out: str,
    clonotype_out: str | None,
) -> list[str]:
    """One invocation: IgBLAST writes a single output format per run."""

    nucleotide = request["sequence_type"] == "nucleotide"
    databases = request["databases"]
    alignments = request["num_alignments"]

    command = [
        binary,
        "-query",
        "query.fasta",
        "-out",
        out,
        "-outfmt",
        outfmt,
        "-organism",
        request["organism"],
        "-ig_seqtype",
        request["ig_seqtype"],
        "-domain_system",
        request["domain_system"],
        "-germline_db_V",
        databases["V"],
        "-num_alignments_V",
        str(alignments["V"]),
        "-min_V_length",
        str(request["min_v_length"]),
        "-num_threads",
        str(request["num_threads"]),
    ]
    if request["extend_align5end"]:
        command.append("-extend_align5end")
    if request["extend_align3end"]:
        command.append("-extend_align3end")
    for option, value in (
        ("-evalue", request["evalue"]),
        ("-word_size", request["word_size"]),
        ("-gapopen", request["gapopen"]),
        ("-gapextend", request["gapextend"]),
    ):
        if value is not None:
            command.extend([option, value])

    if not nucleotide:
        return command

    command.extend(
        [
            "-strand",
            request["strand"],
            "-min_J_length",
            str(request["min_j_length"]),
            "-min_D_match",
            str(request["min_d_match"]),
            "-V_penalty",
            str(request["v_penalty"]),
            "-D_penalty",
            str(request["d_penalty"]),
            "-J_penalty",
            str(request["j_penalty"]),
        ]
    )
    if request["show_translation"]:
        command.append("-show_translation")
    if request["allow_vdj_overlap"]:
        command.append("-allow_vdj_overlap")
    for segment, option in (("D", "-germline_db_D"), ("J", "-germline_db_J")):
        if databases[segment] is not None:
            command.extend(
                [option, databases[segment], f"-num_alignments_{segment}", str(alignments[segment])]
            )
    if databases["C"] is not None:
        command.extend(
            ["-c_region_db", databases["C"], "-num_alignments_C", str(alignments["C"])]
        )

    auxiliary = _auxiliary_data(install_root, request["organism"])
    if auxiliary is not None:
        command.extend(["-auxiliary_data", str(auxiliary)])
    frames = _d_frame_data(install_root, request["organism"])
    if frames is not None:
        command.extend(["-d_frame_data", str(frames)])
    if clonotype_out is not None:
        command.extend(
            ["-num_clonotype", str(request["num_clonotype"]), "-clonotype_out", clonotype_out]
        )
    return command


def run(payload: dict[str, Any]) -> dict[str, Any]:
    request = prepare(payload)
    name = request["name"]
    binary = "igblastn" if request["sequence_type"] == "nucleotide" else "igblastp"
    igblast = executable("IGBLAST_EXECUTABLE", binary)

    install_root = _install_root(igblast)
    if install_root is None:
        raise ToolUnavailable(
            "IgBLAST's internal_data directory was not found next to the executable. "
            "Install the full NCBI distribution, or set IGDATA to its directory."
        )
    # IgBLAST resolves internal_data/ and optional_file/ through IGDATA rather
    # than relative to the binary, so it must be set for every invocation.
    environment = {"IGDATA": str(install_root)}

    fasta = "".join(f">{header}\n{sequence}\n" for header, sequence in request["records"])
    # The AIRR table is the only .tsv a run leaves, so whatever looks for one
    # finds the file the results page offers. The clonotype summary is a report
    # of headed, comment-led blocks rather than a single table, and is named
    # for what it is.
    airr_name = f"{name}.airr.tsv"
    report_name = f"{name}.igblast.txt"
    clonotype_name = f"{name}.clonotypes.txt"

    with tempfile.TemporaryDirectory(prefix="bio-web-igblast-") as temporary:
        workdir = Path(temporary)
        (workdir / "query.fasta").write_text(fasta, encoding="utf-8")

        results: list[dict[str, Any]] = []
        # The AIRR table first, so that the one file the results page offers is
        # written even if a later pass fails, and so the clonotype summary is
        # taken from the pass whose output the reader downloads.
        if request["write_airr"]:
            results.append(
                run_command(
                    _command(
                        request,
                        igblast,
                        install_root,
                        outfmt=AIRR_FORMAT,
                        out=airr_name,
                        clonotype_out=(
                            clonotype_name if request["num_clonotype"] else None
                        ),
                    ),
                    cwd=workdir,
                    env=environment,
                )
            )
        if request["report_format"]:
            results.append(
                run_command(
                    _command(
                        request,
                        igblast,
                        install_root,
                        outfmt=request["report_format"],
                        out=report_name,
                        clonotype_out=(
                            None
                            if request["write_airr"] or not request["num_clonotype"]
                            else clonotype_name
                        ),
                    ),
                    cwd=workdir,
                    env=environment,
                )
            )

        report = ""
        report_path = workdir / report_name
        if report_path.is_file():
            report = report_path.read_text(encoding="utf-8", errors="replace")
        summary = _airr_summary(workdir / airr_name)
        generated = readable_files(workdir)

    result = dict(results[-1])
    return {
        "status": "completed",
        "input": {"fasta": fasta, "queries": len(request["records"])},
        "program": binary,
        "organism": request["organism"],
        "databases": {
            segment: (Path(path).name if path else None)
            for segment, path in request["databases"].items()
        },
        # Every command this run issued, not only the last: a nucleotide run
        # with a readable report is two invocations of IgBLAST.
        "commands": [step["command"] for step in results],
        "summary": summary,
        "report": report,
        "generated_files": generated,
        **result,
    }


def _airr_summary(path: Path) -> dict[str, Any] | None:
    """A few counts read off the AIRR table, for the API's JSON answer.

    The table itself is the result and is served as a file; this is the part a
    client checking a run went as expected reads without downloading it.
    """

    if not path.is_file():
        return None
    try:
        with path.open(encoding="utf-8", errors="replace") as handle:
            header = handle.readline().rstrip("\n").split("\t")
            rows = [line.rstrip("\n").split("\t") for line in handle if line.strip()]
    except OSError:
        return None
    if not header or not rows:
        return {"queries": 0, "productive": 0, "loci": {}}

    index = {column: position for position, column in enumerate(header)}

    def value(row: list[str], column: str) -> str:
        position = index.get(column)
        return row[position] if position is not None and position < len(row) else ""

    loci: dict[str, int] = {}
    for row in rows:
        locus = value(row, "locus") or "unassigned"
        loci[locus] = loci.get(locus, 0) + 1
    return {
        "queries": len(rows),
        "productive": sum(1 for row in rows if value(row, "productive") == "T"),
        "with_cdr3": sum(1 for row in rows if value(row, "cdr3_aa")),
        "loci": dict(sorted(loci.items())),
    }


def check_status() -> ToolStatus:
    try:
        igblastn = executable("IGBLAST_EXECUTABLE", "igblastn")
    except ToolUnavailable as exc:
        return ToolStatus(CheckResult.NOT_INSTALLED, str(exc))
    code, output = probe_command([igblastn, "-version"])
    if code is None:
        return ToolStatus(CheckResult.NOT_INSTALLED, f"Could not run igblastn: {output}")
    if code != 0 or not output:
        return ToolStatus(CheckResult.ERROR, output or f"igblastn -version exited {code}.")
    detail = output.splitlines()[0][:200]

    install_root = _install_root(igblastn)
    if install_root is None:
        return ToolStatus(
            CheckResult.NOT_INSTALLED,
            f"{detail}; internal_data directory not found next to the executable (set IGDATA).",
        )
    installed = available_databases()
    if not installed:
        return ToolStatus(
            CheckResult.NOT_INSTALLED,
            f"{detail}; no germline databases installed under {germline_root()}.",
        )
    # A V and a J set for at least one organism is what a nucleotide run needs;
    # anything less is installed but not usable.
    usable = [
        organism
        for organism, recommended in RECOMMENDED.items()
        if recommended.get("V") in installed and recommended.get("J") in installed
    ]
    if not usable:
        return ToolStatus(
            CheckResult.ERROR,
            f"{detail}; {len(installed)} germline databases are installed under "
            f"{germline_root()}, but no organism has both a V and a J set. "
            "Re-run the installer to download NCBI's prebuilt sets.",
        )
    return ToolStatus(CheckResult.PASS, f"{detail}; germline sets for {', '.join(usable)}")
