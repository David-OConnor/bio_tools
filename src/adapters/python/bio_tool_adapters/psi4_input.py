"""Translate the supported declarative Psithon input to PsiAPI settings."""

from __future__ import annotations

import re
from decimal import Decimal, InvalidOperation
from math import isfinite
from typing import Any, Mapping

from . import ToolInputError, tool_fields
from .field_processing import choice, integer, text

FIELDS = {field.name: field for field in tool_fields("psi4")}
MAX_INPUT = FIELDS["psi4_input"].maxlength
MIB = 2**20
DRIVERS = {
    "energy": ("energy",),
    "optimize": ("optimize",),
    "frequency": ("frequency",),
    "optimize_frequency": ("optimize", "frequency"),
}
OPTIONS = {
    "reference": "reference",
    "scf_type": "scf_type",
    "guess": "scf_guess",
    "freeze_core": "freeze_core",
    "scf__maxiter": "scf_maxiter",
    "scf__e_convergence": "e_convergence",
    "scf__d_convergence": "d_convergence",
    "geom_maxiter": "geom_maxiter",
}
OPTION_ALIASES = {
    "maxiter": "scf__maxiter",
    "e_convergence": "scf__e_convergence",
    "d_convergence": "scf__d_convergence",
    "optking__geom_maxiter": "geom_maxiter",
    "scf__reference": "reference",
    "scf__scf_type": "scf_type",
    "scf__guess": "guess",
    "scf__basis": "basis",
}
IDENT = r"[a-zA-Z_][a-zA-Z_0-9]*"
NUMBER = r"[+-]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:[eE][+-]?[0-9]+)?"
ATOM = r"@?[a-zA-Z]{1,3}[0-9]*"
REF = rf"(?:{ATOM}|[1-9][0-9]*)"
SCALAR = rf"(?:{NUMBER}|[+-]?{IDENT})"
GEOMETRY_ROW = re.compile(
    rf"(?:{ATOM}(?:\s+{SCALAR}){{3}}|{ATOM}(?:\s+{REF}\s+{SCALAR}){{0,3}})",
)
GEOMETRY_DIRECTIVE = re.compile(
    r"(?:units\s+(?:angstrom|ang|au|bohr)|symmetry\s+[a-z][a-z0-9]*|no_com|no_reorient)",
    re.IGNORECASE,
)
CALL = re.compile(
    rf"(?:(?:{IDENT})(?:\s*,\s*{IDENT})?\s*=\s*)?"
    r"(energy|optimize|frequency|frequencies|freq)\s*\(\s*(['\"])"
    r"([a-z0-9+*()./-]+)\2\s*(?:,\s*([^()]*))?\)",
    re.IGNORECASE,
)
MEMORY = re.compile(rf"({NUMBER})\s*(mib|mb|gib|gb)", re.IGNORECASE)
MEMORY_UNITS = {"mb": 10**6, "gb": 10**9, "mib": MIB, "gib": 2**30}


def _lines(document: str):
    for number, line in enumerate(document.splitlines(), start=1):
        if content := line.partition("#")[0].strip():
            yield number, content


def _error(number: int, message: str) -> ToolInputError:
    return ToolInputError(f"Psi4 input line {number}: {message}")


def _geometry(payload: dict[str, Any]) -> str:
    source = text(payload, "coordinates", max_length=FIELDS["coordinates"].maxlength)
    rows = []
    has_atoms = False
    for number, line in _lines(source):
        if any(re.fullmatch(NUMBER, value) and not isfinite(float(value)) for value in line.split()):
            raise ToolInputError(f"Geometry line {number} contains a non-finite number.")
        if GEOMETRY_ROW.fullmatch(line):
            has_atoms = True
        elif not (
            GEOMETRY_DIRECTIVE.fullmatch(line)
            or re.fullmatch(rf"{IDENT}\s*=\s*{NUMBER}", line)
        ):
            raise ToolInputError(f"Unsupported geometry on line {number}: {line[:100]}")
        rows.append(line)
    if not has_atoms:
        raise ToolInputError("Enter at least one atom in the molecular geometry.")
    return "\n".join(rows)


def _integer(payload: dict[str, Any], name: str) -> int:
    field = FIELDS[name]
    value = payload.get(name, field.default)
    if isinstance(value, float) and value.is_integer():
        value = int(value)
    if not re.fullmatch(r"[+-]?[0-9]{1,10}", str(value)):
        raise ToolInputError(f"{field.label} must be an integer.")
    return integer(
        {name: value}, name, default=field.default,
        minimum=field.minimum, maximum=field.maximum,
    )


def _choice(payload: dict[str, Any], name: str) -> str:
    field = FIELDS[name]
    return choice(payload, name, (item.value for item in field.options), field.default)


def _tolerance(value: Any, field: str) -> float:
    value = str(value).strip()
    if not re.fullmatch(NUMBER, value):
        raise ToolInputError(f"{field} must be a number.")
    number = float(value)
    # Psithon accepts an integer n as shorthand for a tolerance of 10**-n.
    if value.isascii() and value.isdigit() and 1 <= number <= 14:
        number = 10 ** -number
    if not 1e-14 <= number <= 0.1:
        raise ToolInputError(f"{field} must be between 1e-14 and 0.1.")
    return number


def _memory_mib(value: Any) -> Decimal:
    field = FIELDS["memory_mib"]
    if not re.fullmatch(NUMBER, str(value)):
        raise ToolInputError("Memory must be a number.")
    try:
        number = Decimal(str(value))
    except InvalidOperation as exc:
        raise ToolInputError("Memory must be a finite number.") from exc
    if not field.minimum <= number <= field.maximum:
        raise ToolInputError(f"Memory must be between {field.minimum} and {field.maximum} MiB.")
    return number


def _validated(payload: dict[str, Any]) -> dict[str, Any]:
    validated = {
        "coordinates": _geometry(payload),
        "molecule_name": text(payload, "molecule_name", required=False, max_length=60),
        **{name: _choice(payload, name) for name in (
            "calculation", "method", "basis", "reference", "scf_type", "scf_guess",
            "freeze_core", "frequency_derivative",
        )},
        **{name: _integer(payload, name) for name in ("charge", "multiplicity", "threads")},
        "memory_mib": _memory_mib(payload.get("memory_mib", FIELDS["memory_mib"].default)),
    }
    if validated["molecule_name"] and not re.fullmatch(IDENT, validated["molecule_name"]):
        raise ToolInputError("The molecule name must be a Psi4 identifier.")
    for name in ("scf_maxiter", "geom_maxiter", "e_convergence", "d_convergence"):
        if payload.get(name) in ("", None):
            validated[name] = ""
        elif name.endswith("maxiter"):
            validated[name] = _integer(payload, name)
        else:
            validated[name] = _tolerance(payload[name], name)
    return validated


def from_boxes(payload: dict[str, Any]) -> str:
    settings = _validated(payload)
    no_option = {"", "auto", "default"}
    options = [f"  basis {settings['basis']}"]
    scf_options = []
    for option, field in OPTIONS.items():
        if settings[field] in no_option:
            continue
        if option.startswith("scf__"):
            scf_options.append(f"  {option.removeprefix('scf__')} {settings[field]}")
        else:
            options.append(f"  {option} {settings[field]}")
    model = settings["method"]
    drivers = []
    for driver in DRIVERS[settings["calculation"]]:
        derivative = settings["frequency_derivative"]
        extra = f", dertype={derivative}" if driver == "frequency" and derivative != "auto" else ""
        drivers.append(f"{driver}('{model}'{extra})")
    name = f" {settings['molecule_name']}" if settings["molecule_name"] else ""
    coordinates = "\n".join(f"  {line}" for line in settings["coordinates"].splitlines())
    scf_block = ["set scf {", *scf_options, "}", ""] if scf_options else []
    return "\n".join([
        f"memory {settings['memory_mib']:f} MiB",
        f"set_num_threads({settings['threads']})",
        "",
        f"molecule{name} {{",
        f"  {settings['charge']} {settings['multiplicity']}",
        coordinates,
        "}",
        "",
        "set {",
        *options,
        "}",
        "",
        *scf_block,
        *drivers,
        "",
    ])


def _block(lines, number: int, label: str):
    for line_number, content in lines:
        if content == "}":
            return
        yield line_number, content
    raise _error(number, f"Close the {label} block with }}.")


def _read_setting(settings: dict[str, Any], line: str, number: int, module: str = ""):
    pair = line.lower().split()
    if len(pair) != 2:
        raise _error(number, "Use one option and value per line.")
    option, value = pair
    option = f"{module}__{option}" if module else option
    option = OPTION_ALIASES.get(option, option)
    field = "basis" if option == "basis" else OPTIONS.get(option)
    if field is None:
        raise _error(number, f"Unsupported setting {option}.")
    if field == "freeze_core":
        value = {"yes": "true", "no": "false", "on": "true", "off": "false"}.get(value, value)
    settings[field] = value


def _read_call(match: re.Match, number: int, settings: dict[str, Any]) -> tuple[str, str, str]:
    driver, _, model, arguments = match.groups()
    driver = "frequency" if driver.lower() in {"freq", "frequencies"} else driver.lower()
    for argument in arguments.split(",") if arguments else ():
        key, equals, value = argument.strip().lower().partition("=")
        key, value = key.strip(), value.strip()
        if equals and key == "return_wfn" and value in {"true", "false"}:
            continue
        if equals and key == "molecule" and value and value == settings.get("molecule_name", "").lower():
            continue
        if equals and key == "dertype" and driver == "frequency" and value in {"0", "1", "2"}:
            settings["frequency_derivative"] = value
            continue
        raise _error(number, "Unsupported calculation argument.")
    method, slash, basis = model.lower().partition("/")
    return driver, "hf" if method == "scf" else method, basis if slash else settings.get("basis", "")


def to_boxes(document: str, _values: Mapping[str, Any] | None = None) -> dict[str, Any]:
    if len(document) > MAX_INPUT:
        raise ToolInputError(f"Psi4 input is too long (maximum {MAX_INPUT} characters).")
    lines = iter(_lines(document))
    settings: dict[str, Any] = {}
    steps = []
    model = None
    for number, line in lines:
        if call := CALL.fullmatch(line):
            if "coordinates" not in settings:
                raise _error(number, "Define the molecule before its calculation.")
            driver, method, basis = _read_call(call, number, settings)
            if not basis:
                raise _error(number, "Give the basis with set basis or as method/basis.")
            if model is not None and model != (method, basis):
                raise _error(number, "Optimization and frequencies must use the same method and basis.")
            model = (method, basis)
            steps.append(driver)
            continue
        if steps:
            raise _error(number, "Put molecule and settings before the calculation calls.")
        if block := re.fullmatch(rf"molecule(?:\s+({IDENT}))?\s*\{{", line, re.IGNORECASE):
            if "coordinates" in settings:
                raise _error(number, "Use one molecule block.")
            rows = [row for _, row in _block(lines, number, "molecule")]
            if rows and re.fullmatch(r"[+-]?[0-9]+\s+[0-9]+", rows[0]):
                settings["charge"], settings["multiplicity"] = rows.pop(0).split()
            settings["molecule_name"] = block[1] or ""
            settings["coordinates"] = "\n".join(rows)
        elif block := re.fullmatch(r"set(?:\s+(scf|optking))?\s*\{", line, re.IGNORECASE):
            for line_number, option in _block(lines, number, "set"):
                _read_setting(settings, option, line_number, (block[1] or "").lower())
        elif setting := re.fullmatch(r"set\s+(.+)", line, re.IGNORECASE):
            _read_setting(settings, setting[1], number)
        elif memory := re.fullmatch(r"memory\s+(.+)", line, re.IGNORECASE):
            value = MEMORY.fullmatch(memory[1])
            if value is None or "e" in value[1].lower():
                raise _error(number, "Use memory with MB, GB, MiB or GiB.")
            settings["memory_mib"] = Decimal(value[1]) * MEMORY_UNITS[value[2].lower()] / MIB
        elif threads := re.fullmatch(r"set_num_threads\(\s*([0-9]+)\s*\)", line, re.IGNORECASE):
            settings["threads"] = threads[1]
        else:
            raise _error(number, "Expected a molecule block, setting, or supported calculation call.")

    calculation = next((name for name, drivers in DRIVERS.items() if list(drivers) == steps), None)
    if calculation is None:
        raise ToolInputError("Use energy(), optimize(), frequency(), or optimize() followed by frequency().")
    settings.update(calculation=calculation, method=model[0], basis=model[1])
    return {name: str(value) for name, value in _validated(settings).items()}


def runtime_settings(document: str) -> dict[str, Any]:
    settings = to_boxes(document)
    options: dict[str, Any] = {}
    for option, field in OPTIONS.items():
        value = settings[field]
        if value in {"", "auto", "default"}:
            continue
        if field.endswith("maxiter"):
            value = int(value)
        elif field.endswith("_convergence"):
            value = float(value)
        elif field == "freeze_core":
            value = value == "true"
        options[option] = value
    if "reference" not in options:
        options["reference"] = "rhf" if settings["multiplicity"] == "1" else "uhf"
    geometry = f"{settings['charge']} {settings['multiplicity']}\n{settings['coordinates']}"
    return {**settings, "geometry": geometry, "options": options}
