"""Run RDKit out of process; write one table and the detail a table cannot hold.

RDKit is an ordinary Python dependency rather than an installed tool, but a run
still goes through a process of its own: that is what gives every run a log, an
output directory the results page can offer a download from, and a failure that
cannot take the web worker with it. The adapter validates the request and
writes it here as JSON; this script does the chemistry.

Input:  a JSON request, as `bio_tool_adapters.rdkit.prepare` builds it.
Output: <output dir>/<job name>.csv, a row per molecule or reaction, and
        <output dir>/summary.json, holding the same values plus the nested
        detail -- a reaction's bond edits, a set's maximum common substructure.
"""

from __future__ import annotations

import argparse
import csv
import json
import re
import sys
from collections import Counter
from pathlib import Path
from typing import Any, Iterable


class InputError(ValueError):
    """The request names something RDKit cannot read. Reported, not raised at."""


MAX_MOLECULES = 5_000
MAX_REACTIONS = 1_000

# The `rdMolHash` functions the Cookbook's molecule-hash recipe reaches for,
# under the names it gives them.
HASHES = (
    "CanonicalSmiles",
    "HetAtomTautomerv2",
    "MurckoScaffold",
    "AnonymousGraph",
    "NetCharge",
)

# An SD file announces itself with the counts line of a connection table, or
# with the record separator between entries.
SDF_MARKERS = ("\n$$$$", "V2000", "V3000")


def _import_rdkit() -> dict[str, Any]:
    """Every RDKit module this adapter uses, imported once and together.

    RDKit is an ordinary dependency rather than an installed tool, so a missing
    one is a deployment problem and is reported as the tool being unavailable.
    """

    try:
        from rdkit import Chem, RDLogger
        from rdkit.Chem import (
            Descriptors,
            rdChemReactions,
            rdFingerprintGenerator,
            rdFMCS,
            rdMolDescriptors,
            rdMolHash,
        )
        from rdkit.Chem.MolStandardize import rdMolStandardize
        from rdkit.Chem.Scaffolds import MurckoScaffold
    except ImportError as exc:
        # Naming the interpreter matters: the usual cause is the parent having
        # launched the wrong one, and the message is the only place the two
        # can be compared.
        raise RuntimeError(
            f"RDKit is not installed in {sys.executable} (prefix {sys.prefix}), "
            "which is the interpreter this analysis was launched with."
        ) from exc

    # RDKit logs a parse failure to stderr; this adapter reports it on the row
    # it belongs to instead, so the run log stays about the run.
    RDLogger.DisableLog("rdApp.error")
    RDLogger.DisableLog("rdApp.warning")
    return {
        "Chem": Chem,
        "Descriptors": Descriptors,
        "MurckoScaffold": MurckoScaffold,
        "rdChemReactions": rdChemReactions,
        "rdFMCS": rdFMCS,
        "rdFingerprintGenerator": rdFingerprintGenerator,
        "rdMolDescriptors": rdMolDescriptors,
        "rdMolHash": rdMolHash,
        "rdMolStandardize": rdMolStandardize,
    }


# --- Input --------------------------------------------------------------------


def _looks_like_sdf(raw: str) -> bool:
    head = raw[:4000]
    return any(marker in head for marker in SDF_MARKERS)


def parse_molecule_input(Chem: Any, raw: str) -> list[tuple[str, str, Any]]:
    """The input as (name, source, molecule) triples; molecule is None if unread.

    SMILES text is read the way `SmilesMolSupplier` reads a `.smi` file: one
    record per line, the SMILES first, an optional name after whitespace, and
    `#` starting a comment. An SD file is read with `SDMolSupplier`, keeping
    each record's `_Name`.
    """

    if not raw.strip():
        raise InputError("Provide at least one molecule.")

    if _looks_like_sdf(raw):
        supplier = Chem.SDMolSupplier()
        supplier.SetData(raw, sanitize=True, removeHs=False, strictParsing=False)
        records: list[tuple[str, str, Any]] = []
        for index, molecule in enumerate(supplier, start=1):
            name = ""
            if molecule is not None and molecule.HasProp("_Name"):
                name = molecule.GetProp("_Name").strip()
            records.append((name or f"mol_{index}", "", molecule))
        if not records:
            raise InputError("No molecules were read from the SD file.")
        if len(records) > MAX_MOLECULES:
            raise InputError(
                f"At most {MAX_MOLECULES:,} molecules are accepted per run."
            )
        return records

    records = []
    for index, line in enumerate(raw.splitlines(), start=1):
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        # A .smi line is the SMILES, then whitespace, then whatever the writer
        # called it -- which may itself contain spaces.
        smiles, _, name = line.partition(" " if " " in line else "\t")
        smiles = smiles.strip()
        name = name.strip() or f"mol_{len(records) + 1}"
        if len(records) >= MAX_MOLECULES:
            raise InputError(
                f"At most {MAX_MOLECULES:,} molecules are accepted per run."
            )
        records.append((name, smiles, Chem.MolFromSmiles(smiles)))
    if not records:
        raise InputError("Provide at least one molecule.")
    return records


def parse_reaction_input(raw: str) -> list[tuple[str, str]]:
    """The input as (name, reaction SMILES) pairs, one per line."""

    records: list[tuple[str, str]] = []
    for line in raw.splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        reaction, _, name = line.partition(" " if " " in line else "\t")
        reaction = reaction.strip()
        if ">" not in reaction:
            raise InputError(
                f'"{reaction[:60]}" is not a reaction SMILES: it needs '
                "reactants>>products, or reactants>agents>products."
            )
        if len(records) >= MAX_REACTIONS:
            raise InputError(
                f"At most {MAX_REACTIONS:,} reactions are accepted per run."
            )
        records.append((name.strip() or f"rxn_{len(records) + 1}", reaction))
    if not records:
        raise InputError("Provide at least one reaction.")
    return records


# --- Molecules ----------------------------------------------------------------


def _canonical(Chem: Any, molecule: Any) -> str:
    return Chem.MolToSmiles(molecule, canonical=True, isomericSmiles=True)


def _common_descriptors(modules: dict[str, Any], molecule: Any) -> dict[str, Any]:
    """The properties an analysis usually starts from, RDKit's own names aside.

    Lipinski's rule of five is counted rather than judged: four criteria, and
    the count of how many the molecule is outside.
    """

    Descriptors = modules["Descriptors"]
    rdMolDescriptors = modules["rdMolDescriptors"]
    weight = Descriptors.MolWt(molecule)
    log_p = Descriptors.MolLogP(molecule)
    donors = rdMolDescriptors.CalcNumHBD(molecule)
    acceptors = rdMolDescriptors.CalcNumHBA(molecule)
    violations = sum(
        (weight > 500, log_p > 5, donors > 5, acceptors > 10),
    )
    return {
        "formula": rdMolDescriptors.CalcMolFormula(molecule),
        "molecular_weight": round(weight, 4),
        "exact_molecular_weight": round(Descriptors.ExactMolWt(molecule), 4),
        "log_p": round(log_p, 4),
        "tpsa": round(Descriptors.TPSA(molecule), 4),
        "h_bond_donors": donors,
        "h_bond_acceptors": acceptors,
        "rotatable_bonds": rdMolDescriptors.CalcNumRotatableBonds(molecule),
        "rings": rdMolDescriptors.CalcNumRings(molecule),
        "aromatic_rings": rdMolDescriptors.CalcNumAromaticRings(molecule),
        "heavy_atoms": molecule.GetNumHeavyAtoms(),
        "fraction_csp3": round(rdMolDescriptors.CalcFractionCSP3(molecule), 4),
        "formal_charge": sum(atom.GetFormalCharge() for atom in molecule.GetAtoms()),
        "lipinski_violations": violations,
    }


def _all_descriptors(modules: dict[str, Any], molecule: Any) -> dict[str, Any]:
    values = modules["Descriptors"].CalcMolDescriptors(molecule)
    return {
        name: (round(value, 6) if isinstance(value, float) else value)
        for name, value in values.items()
    }


def _standardized(modules: dict[str, Any], molecule: Any) -> dict[str, Any]:
    Chem = modules["Chem"]
    standardize = modules["rdMolStandardize"]
    return {
        "cleanup_smiles": _canonical(Chem, standardize.Cleanup(molecule)),
        "fragment_parent_smiles": _canonical(
            Chem, standardize.FragmentParent(molecule)
        ),
        "charge_parent_smiles": _canonical(Chem, standardize.ChargeParent(molecule)),
        "tautomer_parent_smiles": _canonical(
            Chem, standardize.TautomerParent(molecule)
        ),
    }


def _hashes(modules: dict[str, Any], molecule: Any) -> dict[str, Any]:
    rdMolHash = modules["rdMolHash"]
    result = {}
    for name in HASHES:
        function = getattr(rdMolHash.HashFunction, name, None)
        if function is None:
            continue
        try:
            result[f"hash_{_snake(name)}"] = rdMolHash.MolHash(molecule, function)
        except (RuntimeError, ValueError):
            result[f"hash_{_snake(name)}"] = ""
    return result


def _snake(name: str) -> str:
    return re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()


def _scaffold(modules: dict[str, Any], molecule: Any) -> dict[str, Any]:
    Chem = modules["Chem"]
    MurckoScaffold = modules["MurckoScaffold"]
    scaffold = MurckoScaffold.GetScaffoldForMol(molecule)
    return {
        "murcko_scaffold": _canonical(Chem, scaffold),
        "murcko_scaffold_generic": _canonical(
            Chem, MurckoScaffold.MakeScaffoldGeneric(scaffold)
        ),
    }


def _rings(modules: dict[str, Any], molecule: Any) -> dict[str, Any]:
    """Ring counts, including ring *systems* as the Cookbook counts them."""

    Chem = modules["Chem"]
    rings = [set(ring) for ring in Chem.GetSymmSSSR(molecule)]
    # Fused rings share atoms; merging the ones that touch counts each fused
    # system once, which is what the Cookbook's ring-system recipe does.
    systems: list[set[int]] = []
    for ring in rings:
        touching = [system for system in systems if system & ring]
        merged = set(ring).union(*touching) if touching else set(ring)
        systems = [system for system in systems if system not in touching]
        systems.append(merged)
    return {
        "ring_count": len(rings),
        "ring_systems": len(systems),
        "aromatic_ring_count": sum(
            1
            for ring in rings
            if all(molecule.GetAtomWithIdx(index).GetIsAromatic() for index in ring)
        ),
        "aromatic_atoms": sum(1 for atom in molecule.GetAtoms() if atom.GetIsAromatic()),
    }


def _stereo(modules: dict[str, Any], molecule: Any) -> dict[str, Any]:
    Chem = modules["Chem"]
    elements = list(Chem.FindPotentialStereo(molecule))
    unspecified = sum(
        1
        for element in elements
        if str(element.specified) in {"Unspecified", "Unknown"}
    )
    return {
        "stereocenters": sum(
            1 for element in elements if "Atom" in str(element.type)
        ),
        "stereo_bonds": sum(1 for element in elements if "Bond" in str(element.type)),
        "unspecified_stereo": unspecified,
    }


def _fingerprint_generator(modules: dict[str, Any], request: dict[str, Any]) -> Any:
    """The generator for the chosen fingerprint, or None for MACCS and none."""

    generators = modules["rdFingerprintGenerator"]
    size = request["fingerprint_bits"]
    kind = request["fingerprint"]
    if kind == "morgan":
        return generators.GetMorganGenerator(
            radius=request["fingerprint_radius"], fpSize=size
        )
    if kind == "rdkit":
        return generators.GetRDKitFPGenerator(fpSize=size)
    if kind == "atom_pair":
        return generators.GetAtomPairGenerator(fpSize=size)
    if kind == "topological_torsion":
        return generators.GetTopologicalTorsionGenerator(fpSize=size)
    return None


def _fingerprint(
    modules: dict[str, Any], request: dict[str, Any], generator: Any, molecule: Any
) -> dict[str, Any]:
    if request["fingerprint"] == "maccs":
        from rdkit.Chem import MACCSkeys

        bits = MACCSkeys.GenMACCSKeys(molecule)
    else:
        bits = generator.GetFingerprint(molecule)
    return {
        "fingerprint": bits.ToBitString(),
        "fingerprint_on_bits": int(bits.GetNumOnBits()),
    }


def _molecule_row(
    modules: dict[str, Any],
    request: dict[str, Any],
    generator: Any,
    query: Any,
    index: int,
    name: str,
    source: str,
    molecule: Any,
) -> dict[str, Any]:
    Chem = modules["Chem"]
    row: dict[str, Any] = {"index": index, "name": name, "input_smiles": source}
    if molecule is None:
        row["error"] = "RDKit could not parse this molecule."
        return row

    row["canonical_smiles"] = _canonical(Chem, molecule)
    if not source:
        row["input_smiles"] = row["canonical_smiles"]
    try:
        if request["descriptor_set"] == "common":
            row.update(_common_descriptors(modules, molecule))
        elif request["descriptor_set"] == "all":
            row.update(_all_descriptors(modules, molecule))
        if request["standardize"]:
            row.update(_standardized(modules, molecule))
        if request["include_inchi"]:
            row.update(_inchi(modules, molecule))
        if request["hashes"]:
            row.update(_hashes(modules, molecule))
        if request["murcko_scaffold"]:
            row.update(_scaffold(modules, molecule))
        if request["ring_analysis"]:
            row.update(_rings(modules, molecule))
        if request["stereo"]:
            row.update(_stereo(modules, molecule))
        if query is not None:
            matches = molecule.GetSubstructMatches(query)
            row["substructure_matches"] = len(matches)
            row["substructure_first_match"] = (
                " ".join(str(atom) for atom in matches[0]) if matches else ""
            )
        if request["fingerprint"] != "none":
            row.update(_fingerprint(modules, request, generator, molecule))
    except (RuntimeError, ValueError) as error:
        # One calculation failing on one awkward molecule should not lose the
        # columns already computed for it, nor the rest of the set.
        row["error"] = f"{type(error).__name__}: {error}"
    return row


def _inchi(modules: dict[str, Any], molecule: Any) -> dict[str, Any]:
    Chem = modules["Chem"]
    try:
        return {
            "inchi": Chem.MolToInchi(molecule),
            "inchi_key": Chem.MolToInchiKey(molecule),
        }
    except (AttributeError, RuntimeError):
        return {"inchi": "", "inchi_key": "", "inchi_error": "no InChI support"}


def _maximum_common_substructure(
    modules: dict[str, Any], request: dict[str, Any], molecules: list[Any]
) -> dict[str, Any] | None:
    if len(molecules) < 2:
        return None
    result = modules["rdFMCS"].FindMCS(molecules, timeout=request["mcs_timeout"])
    return {
        "smarts": result.smartsString,
        "atoms": result.numAtoms,
        "bonds": result.numBonds,
        "molecules": len(molecules),
        "timed_out": bool(result.canceled),
    }


# --- Reactions ----------------------------------------------------------------


def _molecule_summary(modules: dict[str, Any], molecule: Any) -> dict[str, Any]:
    atoms = list(molecule.GetAtoms())
    return {
        "canonical_smiles": _canonical(modules["Chem"], molecule),
        "formula": modules["rdMolDescriptors"].CalcMolFormula(molecule),
        "formal_charge": sum(atom.GetFormalCharge() for atom in atoms),
        "atom_count": len(atoms),
        "mapped_atom_count": sum(atom.GetAtomMapNum() > 0 for atom in atoms),
    }


def _mapping_inventory(
    molecules: Iterable[Any],
) -> tuple[Counter[int], int, dict[int, str]]:
    counts: Counter[int] = Counter()
    elements: dict[int, str] = {}
    unmapped = 0
    for molecule in molecules:
        for atom in molecule.GetAtoms():
            atom_map = atom.GetAtomMapNum()
            if atom_map <= 0:
                unmapped += 1
                continue
            counts[atom_map] += 1
            elements.setdefault(atom_map, atom.GetSymbol())
    return counts, unmapped, elements


def _mapped_bonds(
    molecules: Iterable[Any],
) -> tuple[dict[tuple[int, int], dict[str, Any]], int]:
    result: dict[tuple[int, int], dict[str, Any]] = {}
    unmapped_bonds = 0
    for molecule in molecules:
        for bond in molecule.GetBonds():
            first = bond.GetBeginAtom()
            second = bond.GetEndAtom()
            first_map = first.GetAtomMapNum()
            second_map = second.GetAtomMapNum()
            if first_map <= 0 or second_map <= 0:
                unmapped_bonds += 1
                continue
            if first_map <= second_map:
                atom_maps = [first_map, second_map]
                elements = [first.GetSymbol(), second.GetSymbol()]
            else:
                atom_maps = [second_map, first_map]
                elements = [second.GetSymbol(), first.GetSymbol()]
            result[(atom_maps[0], atom_maps[1])] = {
                "atom_maps": atom_maps,
                "elements": elements,
                "bond_type": str(bond.GetBondType()).lower(),
                "order": bond.GetBondTypeAsDouble(),
            }
    return result, unmapped_bonds


def _element_and_charge_counts(molecules: Iterable[Any]) -> tuple[Counter[str], int]:
    elements: Counter[str] = Counter()
    charge = 0
    for molecule in molecules:
        for atom in molecule.GetAtoms():
            elements[atom.GetSymbol()] += 1
            if atom.GetSymbol() != "H":
                elements["H"] += atom.GetTotalNumHs()
            charge += atom.GetFormalCharge()
    return elements, charge


def _reaction_analysis(
    modules: dict[str, Any], request: dict[str, Any], name: str, reaction_smiles: str
) -> dict[str, Any]:
    rdChemReactions = modules["rdChemReactions"]
    try:
        reaction = rdChemReactions.ReactionFromSmarts(reaction_smiles, useSmiles=True)
    except (RuntimeError, ValueError) as exc:
        raise InputError(
            f'"{name}" is not a valid reaction SMILES: {exc}'
        ) from exc
    if (
        reaction is None
        or not reaction.GetNumReactantTemplates()
        or not reaction.GetNumProductTemplates()
    ):
        raise InputError(
            f'"{name}" must contain at least one reactant and one product.'
        )

    reactants = list(reaction.GetReactants())
    products = list(reaction.GetProducts())
    agents = list(reaction.GetAgents())

    reactant_maps, reactant_unmapped, reactant_elements = _mapping_inventory(reactants)
    product_maps, product_unmapped, product_elements = _mapping_inventory(products)
    duplicate_reactant_maps = sorted(
        key for key, count in reactant_maps.items() if count > 1
    )
    duplicate_product_maps = sorted(
        key for key, count in product_maps.items() if count > 1
    )
    reactant_map_set = set(reactant_maps)
    product_map_set = set(product_maps)
    missing_from_products = sorted(reactant_map_set - product_map_set)
    missing_from_reactants = sorted(product_map_set - reactant_map_set)
    element_changes = [
        {
            "atom_map": atom_map,
            "reactant_element": reactant_elements[atom_map],
            "product_element": product_elements[atom_map],
        }
        for atom_map in sorted(reactant_map_set & product_map_set)
        if reactant_elements[atom_map] != product_elements[atom_map]
    ]
    mapping_complete = not any(
        (
            reactant_unmapped,
            product_unmapped,
            duplicate_reactant_maps,
            duplicate_product_maps,
            missing_from_products,
            missing_from_reactants,
            element_changes,
        )
    )

    result: dict[str, Any] = {
        "name": name,
        "input_reaction_smiles": reaction_smiles,
        "canonical_reaction_smiles": rdChemReactions.ReactionToSmiles(
            reaction, canonical=True
        ),
        "atom_mapping": {
            "complete": mapping_complete,
            "reactant_unmapped_atom_count": reactant_unmapped,
            "product_unmapped_atom_count": product_unmapped,
            "duplicate_reactant_maps": duplicate_reactant_maps,
            "duplicate_product_maps": duplicate_product_maps,
            "maps_missing_from_products": missing_from_products,
            "maps_missing_from_reactants": missing_from_reactants,
            "element_changes_for_same_map": element_changes,
        },
    }
    warnings: list[str] = []
    if agents:
        warnings.append(
            f'"{name}": agent templates are reported but excluded from atom balance '
            "and bond edits."
        )
    if not mapping_complete:
        warnings.append(
            f'"{name}": atom mapping is incomplete or inconsistent; bond edits are '
            "provisional."
        )

    if request["reaction_participants"]:
        result["participants"] = {
            "reactants": [_molecule_summary(modules, m) for m in reactants],
            "agents": [_molecule_summary(modules, m) for m in agents],
            "products": [_molecule_summary(modules, m) for m in products],
        }

    if request["reaction_balance"]:
        reactant_counts, reactant_charge = _element_and_charge_counts(reactants)
        product_counts, product_charge = _element_and_charge_counts(products)
        element_delta = {
            element: product_counts[element] - reactant_counts[element]
            for element in sorted(set(reactant_counts) | set(product_counts))
            if product_counts[element] != reactant_counts[element]
        }
        charge_delta = product_charge - reactant_charge
        result["balance"] = {
            "balanced": not element_delta and charge_delta == 0,
            "reactant_elements": dict(sorted(reactant_counts.items())),
            "product_elements": dict(sorted(product_counts.items())),
            "product_minus_reactant_elements": element_delta,
            "reactant_formal_charge": reactant_charge,
            "product_formal_charge": product_charge,
            "product_minus_reactant_charge": charge_delta,
        }
        if element_delta or charge_delta:
            warnings.append(
                f'"{name}": the parsed reaction is not element- and charge-balanced, '
                "including implicit hydrogens."
            )

    if request["reaction_bond_edits"]:
        reactant_bonds, reactant_unmapped_bonds = _mapped_bonds(reactants)
        product_bonds, product_unmapped_bonds = _mapped_bonds(products)
        reactant_bond_keys = set(reactant_bonds)
        product_bond_keys = set(product_bonds)
        broken = [
            reactant_bonds[key] for key in sorted(reactant_bond_keys - product_bond_keys)
        ]
        formed = [
            product_bonds[key] for key in sorted(product_bond_keys - reactant_bond_keys)
        ]
        order_changed = [
            {
                "atom_maps": list(key),
                "elements": product_bonds[key]["elements"],
                "reactant_bond_type": reactant_bonds[key]["bond_type"],
                "reactant_order": reactant_bonds[key]["order"],
                "product_bond_type": product_bonds[key]["bond_type"],
                "product_order": product_bonds[key]["order"],
            }
            for key in sorted(reactant_bond_keys & product_bond_keys)
            if reactant_bonds[key]["bond_type"] != product_bonds[key]["bond_type"]
            or reactant_bonds[key]["order"] != product_bonds[key]["order"]
        ]
        centre = sorted(
            {
                atom_map
                for edit in [*broken, *formed, *order_changed]
                for atom_map in edit["atom_maps"]
            }
        )
        result["bond_edits"] = {
            "reliable": mapping_complete,
            "formed": formed,
            "broken": broken,
            "order_changed": order_changed,
            "bonds_skipped_for_unmapped_atoms": {
                "reactants": reactant_unmapped_bonds,
                "products": product_unmapped_bonds,
            },
        }
        result["reaction_center_atom_maps"] = centre
        if mapping_complete and not centre:
            warnings.append(
                f'"{name}": no covalent bond edits were found between the mapped sides.'
            )

    result["warnings"] = warnings
    return result


def _reaction_row(analysis: dict[str, Any], index: int) -> dict[str, Any]:
    """One reaction as a CSV row; the nested detail stays in the JSON."""

    mapping = analysis["atom_mapping"]
    edits = analysis.get("bond_edits") or {}
    balance = analysis.get("balance") or {}
    participants = analysis.get("participants") or {}
    row: dict[str, Any] = {
        "index": index,
        "name": analysis["name"],
        "canonical_reaction_smiles": analysis["canonical_reaction_smiles"],
        "mapping_complete": mapping["complete"],
    }
    if participants:
        row.update(
            {
                "reactants": len(participants["reactants"]),
                "agents": len(participants["agents"]),
                "products": len(participants["products"]),
            }
        )
    if balance:
        row.update(
            {
                "balanced": balance["balanced"],
                "charge_change": balance["product_minus_reactant_charge"],
                "element_change": " ".join(
                    f"{element}{value:+d}"
                    for element, value in balance[
                        "product_minus_reactant_elements"
                    ].items()
                ),
            }
        )
    if edits:
        row.update(
            {
                "bonds_formed": len(edits["formed"]),
                "bonds_broken": len(edits["broken"]),
                "bond_orders_changed": len(edits["order_changed"]),
                "reaction_center_atom_maps": " ".join(
                    str(value) for value in analysis["reaction_center_atom_maps"]
                ),
            }
        )
    row["warnings"] = " | ".join(analysis["warnings"])
    return row


def _write_csv(path: Path, rows: list[dict[str, Any]]) -> None:
    """One CSV whose columns are the union of the rows', in first-seen order.

    A row that failed carries fewer keys than one that succeeded, so the header
    is built from every row rather than from the first.
    """

    columns: list[str] = []
    for row in rows:
        for key in row:
            if key not in columns:
                columns.append(key)
    with path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=columns, extrasaction="ignore")
        writer.writeheader()
        for row in rows:
            writer.writerow(row)


def analyze(request: dict[str, Any], output_dir: Path) -> dict[str, Any]:
    modules = _import_rdkit()
    Chem = modules["Chem"]
    output_dir.mkdir(parents=True, exist_ok=True)

    if request["task"] == "reaction":
        analyses = [
            _reaction_analysis(modules, request, name, reaction)
            for name, reaction in parse_reaction_input(request["input"])
        ]
        rows = [
            _reaction_row(analysis, index)
            for index, analysis in enumerate(analyses, start=1)
        ]
        summary: dict[str, Any] = {
            "task": "reaction",
            "reactions": len(analyses),
            "mapping_complete": sum(
                1 for entry in analyses if entry["atom_mapping"]["complete"]
            ),
            "balanced": sum(
                1 for entry in analyses if (entry.get("balance") or {}).get("balanced")
            ),
        }
        # Named apart from the count in the summary above: one is how many
        # reactions were read, the other is every one of them in full.
        detail: dict[str, Any] = {"reaction_analyses": analyses}
        warnings = [note for entry in analyses for note in entry["warnings"]]
    else:
        query = None
        if request["substructure_smarts"]:
            query = Chem.MolFromSmarts(request["substructure_smarts"])
            if query is None:
                raise InputError(
                    "substructure_smarts is not a valid SMARTS pattern: "
                    f'"{request["substructure_smarts"][:80]}".'
                )
        generator = _fingerprint_generator(modules, request)
        records = parse_molecule_input(Chem, request["input"])
        rows = [
            _molecule_row(
                modules, request, generator, query, index, name, source, molecule
            )
            for index, (name, source, molecule) in enumerate(records, start=1)
        ]
        parsed = [molecule for _, _, molecule in records if molecule is not None]
        if not parsed:
            raise InputError(
                "RDKit could not parse any of the molecules in this input."
            )
        summary = {
            "task": "molecule",
            "molecules": len(records),
            "parsed": len(parsed),
            "failed": len(records) - len(parsed),
        }
        detail = {}
        warnings = []
        if summary["failed"]:
            warnings.append(
                f"{summary['failed']} of {summary['molecules']} molecules could not be "
                "parsed; each is a row in the table with its error."
            )
        if request["find_mcs"]:
            found = _maximum_common_substructure(modules, request, parsed)
            detail["maximum_common_substructure"] = found
            if found is None:
                warnings.append(
                    "A maximum common substructure needs at least two molecules that "
                    "parsed."
                )
            elif found["timed_out"]:
                warnings.append(
                    "The maximum common substructure search timed out; what is reported "
                    "is the largest substructure found before it stopped."
                )
        if query is not None:
            summary["substructure_hits"] = sum(
                1 for row in rows if row.get("substructure_matches")
            )

    table = output_dir / f"{request['name']}.csv"
    _write_csv(table, rows)
    summary["output_csv"] = table.name
    summary["row_count"] = len(rows)
    summary["columns"] = list(rows[0]) if rows else []
    summary["warnings"] = warnings
    (output_dir / "summary.json").write_text(
        json.dumps({**summary, **detail, "rows": rows}, indent=2, allow_nan=False),
        encoding="utf-8",
    )
    print(
        f"Wrote {len(rows)} rows to {table.name} "
        f"({len(summary['columns'])} columns).",
        flush=True,
    )
    return summary


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    args = parser.parse_args()
    request = json.loads(args.input.read_text(encoding="utf-8"))
    try:
        analyze(request, args.output_dir)
    except InputError as error:
        raise SystemExit(f"invalid input: {error}")


if __name__ == "__main__":
    main()
