"""Prepare AlphaFold 3 inputs for `run_alphafold.py`.

Runs inside AlphaFold 3's own environment, so the input is read by AlphaFold 3's own parser
(`folding_input.load_fold_inputs_from_path`): it is validated exactly as the run will validate
it, an AlphaFold Server job list is converted to the `alphafold3` dialect, and any MSA, template,
or user CCD path is read inline. Then each input gets whatever MSAs and templates the chosen
source provides, for the entities that do not already give their own:

- `colabfold`: protein MSAs from the ColabFold MMseqs2 API. Every distinct protein sequence is
  searched once (`env` mode: UniRef plus the environmental databases). In an input with several
  distinct proteins, their paired alignment (`pairgreedy`) is placed first in each chain's
  `unpairedMsa`, row for row, with `pairedMsa` empty -- the custom-pairing route the input docs
  recommend, since AlphaFold 3 pairs `pairedMsa` rows by UniProt species, which ColabFold's hits
  do not carry. RNA runs MSA-free, and no templates are used.
- `none`: every protein and RNA chain runs MSA-free and template-free.
- `databases`: nothing is filled in; AlphaFold 3's own data pipeline searches for what is missing.

Usage:

    alphafold3_prepare.py INPUT OUTPUT_DIR SUMMARY --msa {colabfold,none,databases}
        [--host URL] [--num-seeds N]

Writes one `alphafold3`-dialect JSON per fold input into OUTPUT_DIR, which is what
`run_alphafold.py --input_dir` reads, and a SUMMARY JSON describing what was done. An input
AlphaFold 3 refuses exits 2, and an MSA server failure 3, each with the reason in SUMMARY's
"error".

Input format: https://github.com/google-deepmind/alphafold3/blob/v3.0.4/docs/input.md
MSA API: https://github.com/sokrypton/ColabFold/blob/main/colabfold/colabfold.py (run_mmseqs2)
"""

from __future__ import annotations

import argparse
import io
import json
import random
import sys
import tarfile
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any

from alphafold3.common import folding_input

USER_AGENT = "bio_tools/alphafold3"

# AlphaFold 3 builds no MSA for a chain this short, so it is never sent to the server.
MINIMUM_MSA_LENGTH = 5

# How long one search may wait on the server, queueing included.
SEARCH_TIMEOUT = 3 * 60 * 60


class InputError(ValueError):
    """AlphaFold 3 would refuse this input."""


class MsaServerError(RuntimeError):
    """The MSA server did not produce alignments."""


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("input", type=Path)
    parser.add_argument("output_dir", type=Path)
    parser.add_argument("summary", type=Path)
    parser.add_argument("--msa", choices=("colabfold", "none", "databases"), required=True)
    parser.add_argument("--host", default="https://api.colabfold.com")
    parser.add_argument("--num-seeds", type=int)
    arguments = parser.parse_args()

    try:
        summary = prepare(arguments)
    except InputError as exc:
        _write(arguments.summary, {"error": str(exc), "kind": "input"})
        print(f"Invalid AlphaFold 3 input: {exc}", file=sys.stderr)
        raise SystemExit(2)
    except MsaServerError as exc:
        _write(arguments.summary, {"error": str(exc), "kind": "msa_server"})
        print(f"MSA server error: {exc}", file=sys.stderr)
        raise SystemExit(3)
    _write(arguments.summary, summary)


def prepare(arguments: argparse.Namespace) -> dict[str, Any]:
    try:
        inputs = list(folding_input.load_fold_inputs_from_path(arguments.input))
    except (ValueError, KeyError, TypeError, IndexError, OSError) as exc:
        raise InputError(str(exc) or type(exc).__name__) from exc
    if not inputs:
        raise InputError("The input holds no fold jobs.")

    documents: list[dict[str, Any]] = []
    names: dict[str, str] = {}
    for fold_input in inputs:
        if not fold_input.chains:
            raise InputError(f'Fold job "{fold_input.name}" has no chains.')
        sanitised = fold_input.sanitised_name()
        if not sanitised:
            raise InputError(
                f'Fold job name "{fold_input.name}" has no letters, digits, "_", "-" or "."'
                " to name its output directory with."
            )
        if sanitised in names:
            raise InputError(
                f'Fold jobs "{names[sanitised]}" and "{fold_input.name}" would both write to '
                f'the output directory "{sanitised}"; give them distinct names.'
            )
        names[sanitised] = fold_input.name
        if arguments.num_seeds is not None and len(fold_input.rng_seeds) != 1:
            raise InputError(
                f'Number of seeds needs exactly one seed in each input, and "{fold_input.name}" '
                f"gives {len(fold_input.rng_seeds)}."
            )
        documents.append(json.loads(fold_input.to_json()))

    warnings: list[str] = []
    paired = False
    if arguments.msa == "colabfold":
        paired = _fill_from_colabfold(documents, arguments.host.rstrip("/"), warnings)
    elif arguments.msa == "none":
        for document in documents:
            _fill_empty(document)

    arguments.output_dir.mkdir(parents=True, exist_ok=True)
    jobs = []
    for index, (document, sanitised) in enumerate(zip(documents, names), start=1):
        path = arguments.output_dir / f"{index:02d}_{sanitised}.json"
        path.write_text(json.dumps(document, indent=2), encoding="utf-8")
        jobs.append(
            {
                "name": document["name"],
                "sanitised": sanitised,
                "file": path.name,
                "seeds": document["modelSeeds"],
                "entities": [next(iter(entry)) for entry in document["sequences"]],
            }
        )
    return {"jobs": jobs, "msa": arguments.msa, "paired": paired, "warnings": warnings}


def _entities(document: dict[str, Any]):
    for entry in document["sequences"]:
        kind, entity = next(iter(entry.items()))
        yield kind, entity


def _fill_empty(document: dict[str, Any]) -> None:
    """Run every chain the input leaves without MSAs or templates MSA- and template-free."""

    for kind, entity in _entities(document):
        if kind == "protein":
            _complete_pair(entity)
            if entity.get("templates") is None:
                entity["templates"] = []
        elif kind == "rna" and entity.get("unpairedMsa") is None:
            entity["unpairedMsa"] = ""


def _complete_pair(entity: dict[str, Any]) -> None:
    """Set whichever of a protein's two MSAs is missing to empty.

    AlphaFold 3 takes the two together or not at all; a chain given one of them is taken to have
    meant the other to be empty, which is also what the input docs recommend for a custom MSA.
    """

    for key in ("unpairedMsa", "pairedMsa"):
        if entity.get(key) is None:
            entity[key] = ""


def _fill_from_colabfold(
    documents: list[dict[str, Any]], host: str, warnings: list[str]
) -> bool:
    """Search the ColabFold server for every protein MSA the inputs leave missing."""

    wanted: dict[str, None] = {}
    for document in documents:
        for kind, entity in _entities(document):
            if (
                kind == "protein"
                and entity.get("unpairedMsa") is None
                and entity.get("pairedMsa") is None
                and len(entity["sequence"]) >= MINIMUM_MSA_LENGTH
            ):
                wanted[entity["sequence"]] = None
    sequences = list(wanted)

    unpaired: dict[str, list[tuple[str, str]]] = {}
    if sequences:
        print(
            f"Searching {len(sequences)} protein sequence(s) on {host} ...", flush=True
        )
        results = _search(host, sequences, pairing=False)
        unpaired = {sequence: results[index] for index, sequence in enumerate(sequences)}

    any_paired = False
    rna_without_msa = False
    for document in documents:
        targets = [
            entity
            for kind, entity in _entities(document)
            if kind == "protein" and entity["sequence"] in unpaired
            and entity.get("unpairedMsa") is None
            and entity.get("pairedMsa") is None
        ]
        distinct = list(dict.fromkeys(entity["sequence"] for entity in targets))
        paired: dict[str, list[tuple[str, str]]] = {}
        if len(distinct) > 1:
            print(
                f'Pairing {len(distinct)} distinct proteins of "{document["name"]}" ...',
                flush=True,
            )
            results = _search(host, distinct, pairing=True)
            rows = {len(results[index]) for index in range(len(distinct))}
            if len(rows) == 1:
                paired = {sequence: results[index] for index, sequence in enumerate(distinct)}
                any_paired = True
            else:
                warnings.append(
                    f'The MSA server\'s paired alignments for "{document["name"]}" did not have '
                    "one row per chain, so its chains were given unpaired MSAs only."
                )
        for entity in targets:
            sequence = entity["sequence"]
            records = [(">query", sequence)]
            records += paired.get(sequence, [])[1:]
            records += unpaired[sequence][1:]
            entity["unpairedMsa"] = "".join(f"{header}\n{row}\n" for header, row in records)
            entity["pairedMsa"] = ""

        for kind, entity in _entities(document):
            if kind == "protein":
                _complete_pair(entity)
                if entity.get("templates") is None:
                    entity["templates"] = []
            elif kind == "rna" and entity.get("unpairedMsa") is None:
                entity["unpairedMsa"] = ""
                rna_without_msa = True

    if rna_without_msa:
        warnings.append(
            "The ColabFold server has no RNA databases, so RNA chains without an MSA of their own "
            "ran MSA-free."
        )
    return any_paired


def _search(host: str, sequences: list[str], *, pairing: bool) -> list[list[tuple[str, str]]]:
    """One ColabFold MMseqs2 search: each sequence's alignment, query first, in input order."""

    query = "".join(f">{101 + index}\n{sequence}\n" for index, sequence in enumerate(sequences))
    endpoint, mode = ("ticket/pair", "pairgreedy") if pairing else ("ticket/msa", "env")
    body = urllib.parse.urlencode({"q": query, "mode": mode}).encode()

    def submit() -> dict[str, Any]:
        return json.loads(_request(f"{host}/{endpoint}", body))

    ticket = submit()
    while ticket.get("status") in ("UNKNOWN", "RATELIMIT"):
        time.sleep(5 + random.randint(0, 5))
        ticket = submit()
    _check(ticket)

    started = time.monotonic()
    while ticket.get("status") in ("UNKNOWN", "RUNNING", "PENDING"):
        if time.monotonic() - started > SEARCH_TIMEOUT:
            raise MsaServerError(
                f"The search did not finish within {SEARCH_TIMEOUT // 3600} hours "
                f"(ticket {ticket.get('id')}, status {ticket.get('status')})."
            )
        time.sleep(5 + random.randint(0, 5))
        ticket = json.loads(_request(f"{host}/ticket/{ticket['id']}"))
    _check(ticket)
    if ticket.get("status") != "COMPLETE":
        raise MsaServerError(f"The search ended with status {ticket.get('status')!r}.")

    archive = _request(f"{host}/result/download/{ticket['id']}", timeout=600)
    names = ("pair.a3m",) if pairing else ("uniref.a3m", "bfd.mgnify30.metaeuk30.smag30.a3m")
    blocks: dict[int, list[tuple[str, str]]] = {}
    try:
        with tarfile.open(fileobj=io.BytesIO(archive), mode="r:gz") as tar:
            for name in names:
                member = tar.extractfile(name)
                if member is None:
                    raise MsaServerError(f"The result archive has no {name}.")
                text = member.read().decode("utf-8", errors="replace")
                for key, records in _a3m_blocks(text).items():
                    existing = blocks.setdefault(key, [])
                    # Each database's block restates the query; the first one is kept.
                    existing.extend(records if not existing else records[1:])
    except (tarfile.TarError, KeyError) as exc:
        raise MsaServerError(f"The result archive could not be read: {exc}") from exc

    alignments = []
    for index, sequence in enumerate(sequences):
        records = blocks.get(101 + index)
        if not records:
            raise MsaServerError(f"The server returned no alignment for sequence {index + 1}.")
        if records[0][1].replace("-", "").upper() != sequence.upper():
            raise MsaServerError(
                f"The server's alignment for sequence {index + 1} does not start with it."
            )
        alignments.append(records)
    return alignments


def _check(ticket: dict[str, Any]) -> None:
    status = ticket.get("status")
    if status == "ERROR":
        raise MsaServerError(
            "The server rejected the search. Check that every protein sequence uses standard "
            "amino-acid letters; if it persists, the server may be failing, so try again later."
        )
    if status == "MAINTENANCE":
        raise MsaServerError("The server is down for maintenance; try again in a few minutes.")


def _a3m_blocks(text: str) -> dict[int, list[tuple[str, str]]]:
    """ColabFold's combined A3M, split into each query's records.

    Every query's alignment starts with a header naming it (`>101`), and each one after the
    first is preceded by a NUL byte.
    """

    blocks: dict[int, list[tuple[str, str]]] = {}
    current: list[tuple[str, str]] | None = None
    header: str | None = None
    rows: list[str] = []
    new_block = True

    def flush() -> None:
        if current is not None and header is not None:
            current.append((header, "".join(rows)))

    for line in text.splitlines():
        if "\x00" in line:
            line = line.replace("\x00", "")
            new_block = True
        line = line.strip()
        if not line:
            continue
        if line.startswith(">"):
            flush()
            rows = []
            header = line
            if new_block:
                try:
                    key = int(line[1:].split()[0])
                except (ValueError, IndexError):
                    raise MsaServerError(f"Unexpected alignment header {line!r}.") from None
                current = blocks.setdefault(key, [])
                new_block = False
        else:
            rows.append(line)
    flush()
    return blocks


def _request(url: str, data: bytes | None = None, *, timeout: int = 60) -> bytes:
    request = urllib.request.Request(url, data=data, headers={"User-Agent": USER_AGENT})
    for attempt in range(1, 7):
        try:
            with urllib.request.urlopen(request, timeout=timeout) as response:
                return response.read()
        except urllib.error.HTTPError as exc:
            if exc.code < 500 and exc.code != 429:
                detail = exc.read().decode("utf-8", errors="replace").strip()[:500]
                raise MsaServerError(f"{url} answered HTTP {exc.code}: {detail}") from exc
            error: Exception = exc
        except (urllib.error.URLError, TimeoutError, ConnectionError) as exc:
            error = exc
        if attempt == 6:
            raise MsaServerError(f"Could not reach {url}: {error}") from error
        print(f"Retrying {url} after: {error}", file=sys.stderr, flush=True)
        time.sleep(5 * attempt)
    raise AssertionError("unreachable")


def _write(path: Path, value: dict[str, Any]) -> None:
    path.write_text(json.dumps(value, indent=2), encoding="utf-8")


if __name__ == "__main__":
    main()
