"""Psi4's form: written as a Psithon input or a PsiAPI script, and read back.

Psithon and PsiAPI are Psi4's two input languages
(https://psicode.org/psi4manual/master/tutorial.html). Raw and uploaded inputs
run as written; this module only serves "Set parameters here".
"""

from __future__ import annotations

import ast
import re
from decimal import Decimal, InvalidOperation
from math import isfinite
from typing import Any, Mapping

from . import ToolInputError, tool_fields
from .field_processing import choice, integer, text

FIELDS = {field.name: field for field in tool_fields("psi4")}
MAX_INPUT = FIELDS["psi4_input"].maxlength
MIB = 2**20
PSITHON = "psithon"
PSIAPI = "psiapi"

# Driver calls for each calculation, in order.
DRIVERS = {
    "energy": ("energy",),
    "gradient": ("gradient",),
    "properties": ("properties",),
    "optimize": ("optimize",),
    "frequency": ("frequency",),
    "hessian": ("hessian",),
    "optimize_frequency": ("optimize", "frequency"),
}
# Psi4's aliases for its driver functions.
DRIVER_ALIASES = {
    "opt": "optimize",
    "freq": "frequency",
    "frequencies": "frequency",
    "prop": "properties",
}
# Options with their own field, keyed by (module, option). An empty module is
# a global option.
OPTION_FIELDS = {
    ("", "basis"): "basis",
    ("", "reference"): "reference",
    ("scf", "reference"): "reference",
    ("", "scf_type"): "scf_type",
    ("scf", "scf_type"): "scf_type",
    ("", "guess"): "scf_guess",
    ("scf", "guess"): "scf_guess",
    ("", "freeze_core"): "freeze_core",
    ("scf", "maxiter"): "scf_maxiter",
    ("scf", "e_convergence"): "e_convergence",
    ("scf", "d_convergence"): "d_convergence",
    ("", "geom_maxiter"): "geom_maxiter",
    ("optking", "geom_maxiter"): "geom_maxiter",
}
# Where each field's option is written.
FIELD_OPTIONS = {
    "basis": ("", "basis"),
    "reference": ("", "reference"),
    "scf_type": ("", "scf_type"),
    "scf_guess": ("", "guess"),
    "freeze_core": ("", "freeze_core"),
    "scf_maxiter": ("scf", "maxiter"),
    "e_convergence": ("scf", "e_convergence"),
    "d_convergence": ("scf", "d_convergence"),
    "geom_maxiter": ("", "geom_maxiter"),
}
UNSET = {"", "auto", "default"}

IDENT = r"[A-Za-z_][A-Za-z_0-9]*"
NUMBER = r"[+-]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:[eE][+-]?[0-9]+)?"
# Method and basis strings, including CBS forms such as mp2/cc-pv[tq]z.
MODEL = re.compile(r"[A-Za-z0-9()\[\]+*,./_-]+")
OPTION_LINE = re.compile(rf"({IDENT})\s+(\S.*)")
MEMORY = re.compile(rf"({NUMBER})\s*([kmgt]i?b|b)?", re.IGNORECASE)
MEMORY_UNITS = {
    "b": 1, "kb": 10**3, "mb": 10**6, "gb": 10**9, "tb": 10**12,
    "kib": 2**10, "mib": 2**20, "gib": 2**30, "tib": 2**40,
}
NOT_A_FORM = (
    "This input does more than the form can show. Keep editing it as raw input."
)


# --- Validation -----------------------------------------------------------------


def _optional_integer(payload: Mapping[str, Any], name: str) -> int | None:
    value = payload.get(name)
    if value in ("", None):
        return None
    field = FIELDS[name]
    if isinstance(value, float) and value.is_integer():
        value = int(value)
    if not re.fullmatch(r"[+-]?[0-9]{1,10}", str(value).strip()):
        raise ToolInputError(f"{field.label} must be an integer.")
    return integer(
        {name: value}, name, default=0, minimum=field.minimum, maximum=field.maximum,
    )


def _tolerance(payload: Mapping[str, Any], name: str) -> float | None:
    value = str(payload.get(name) or "").strip()
    if not value:
        return None
    label = FIELDS[name].label
    if not re.fullmatch(NUMBER, value):
        raise ToolInputError(f"{label} must be a number.")
    number = float(value)
    # Psi4 reads an integer n as 10**-n.
    if value.isdigit() and 1 <= number <= 14:
        number = 10 ** -number
    if not 1e-14 <= number <= 0.1:
        raise ToolInputError(f"{label} must be between 1e-14 and 0.1.")
    return number


def _choice(payload: Mapping[str, Any], name: str) -> str:
    field = FIELDS[name]
    return choice(payload, name, (item.value for item in field.options), field.default)


def memory_mib(payload: Mapping[str, Any]) -> Decimal:
    field = FIELDS["memory_mib"]
    value = payload.get("memory_mib", field.default)
    try:
        number = Decimal(str(value).strip())
    except InvalidOperation as exc:
        raise ToolInputError("Memory must be a number.") from exc
    if not number.is_finite() or not field.minimum <= number <= field.maximum:
        raise ToolInputError(f"Memory must be between {field.minimum} and {field.maximum} MiB.")
    return number


def threads(payload: Mapping[str, Any]) -> int:
    field = FIELDS["threads"]
    return integer(
        payload, "threads", default=field.default,
        minimum=field.minimum, maximum=field.maximum,
    )


def input_format(payload: Mapping[str, Any]) -> str:
    return _choice(payload, "input_format")


def _model(payload: Mapping[str, Any], name: str, required: bool) -> str:
    value = text(payload, name, required=required, max_length=200)
    if value and not MODEL.fullmatch(value):
        raise ToolInputError(
            f"{FIELDS[name].label} may contain letters, digits and ()[]+*,./_- only."
        )
    return value


def _extra_options(payload: Mapping[str, Any]) -> list[tuple[str, str, str]]:
    """(module, option, value) for each line of the additional options."""

    source = text(payload, "extra_options", required=False, max_length=FIELDS["extra_options"].maxlength)
    options = []
    for number, line in enumerate(source.splitlines(), start=1):
        line = line.partition("#")[0].strip()
        if not line:
            continue
        match = OPTION_LINE.fullmatch(line)
        if match is None or match[2].rstrip().endswith("{"):
            raise ToolInputError(
                f"Additional options line {number}: use option value, "
                "or module__option value."
            )
        module, _, option = match[1].lower().rpartition("__")
        options.append((module, option, match[2].strip()))
    return options


def _validated(payload: Mapping[str, Any]) -> dict[str, Any]:
    coordinates = text(payload, "coordinates", max_length=FIELDS["coordinates"].maxlength)
    if "}" in coordinates or '"""' in coordinates:
        raise ToolInputError("The molecular geometry cannot contain } or \"\"\".")
    name = text(payload, "molecule_name", required=False, max_length=60)
    if name and not re.fullmatch(IDENT, name):
        raise ToolInputError("The molecule name must be a Psi4 identifier.")
    properties = re.split(r"[\s,]+", text(payload, "properties", required=False, max_length=2000))
    properties = [item.lower() for item in properties if item]
    if any(not re.fullmatch(IDENT, item) for item in properties):
        raise ToolInputError("List properties by name, for example dipole, mulliken_charges.")
    settings = {
        "input_format": input_format(payload),
        "coordinates": coordinates,
        "molecule_name": name,
        "charge": _optional_integer(payload, "charge"),
        "multiplicity": _optional_integer(payload, "multiplicity"),
        "calculation": _choice(payload, "calculation"),
        "method": _model(payload, "method", required=True),
        "basis": _model(payload, "basis", required=False),
        "properties": properties,
        "extra_options": _extra_options(payload),
        "scf_maxiter": _optional_integer(payload, "scf_maxiter"),
        "geom_maxiter": _optional_integer(payload, "geom_maxiter"),
        "e_convergence": _tolerance(payload, "e_convergence"),
        "d_convergence": _tolerance(payload, "d_convergence"),
        "threads": threads(payload),
        "memory_mib": memory_mib(payload),
    }
    for name in ("reference", "scf_type", "scf_guess", "freeze_core", "frequency_derivative"):
        settings[name] = _choice(payload, name)
    if (settings["charge"] is None) != (settings["multiplicity"] is None):
        raise ToolInputError("Give both the charge and the multiplicity, or neither.")
    if settings["properties"] and settings["calculation"] != "properties":
        raise ToolInputError("Properties are computed by the Properties calculation.")
    return settings


# --- Writing --------------------------------------------------------------------


def _options(settings: Mapping[str, Any]) -> list[tuple[str, str, Any]]:
    """(module, option, value) for every option the form sets, in order."""

    options = []
    for field, (module, option) in FIELD_OPTIONS.items():
        value = settings[field]
        if value is None or value in UNSET:
            continue
        options.append((module, option, value))
    return options + settings["extra_options"]


def _psithon_value(value: Any) -> str:
    return format(value, "g") if isinstance(value, float) else str(value)


def _python_value(value: Any) -> Any:
    """A Psithon option value as the Python value PsiAPI expects."""

    if not isinstance(value, str):
        return value
    lowered = value.lower()
    if lowered in {"true", "false"}:
        return lowered == "true"
    if re.fullmatch(r"[+-]?[0-9]+", value):
        return int(value)
    if re.fullmatch(NUMBER, value):
        return float(value)
    if value.startswith("["):
        try:
            return ast.literal_eval(value)
        except (ValueError, SyntaxError):
            pass
    return value


def _calls(settings: Mapping[str, Any]) -> list[tuple[str, str, list[tuple[str, Any]]]]:
    method = settings["method"]
    calls = []
    for driver in DRIVERS[settings["calculation"]]:
        arguments = []
        if driver == "frequency" and settings["frequency_derivative"] != "auto":
            arguments.append(("dertype", int(settings["frequency_derivative"])))
        if driver == "properties" and settings["properties"]:
            arguments.append(("properties", settings["properties"]))
        calls.append((driver, method, arguments))
    return calls


def _geometry_lines(settings: Mapping[str, Any]) -> list[str]:
    lines = []
    if settings["charge"] is not None:
        lines.append(f"{settings['charge']} {settings['multiplicity']}")
    return lines + settings["coordinates"].splitlines()


def _psithon(settings: Mapping[str, Any]) -> str:
    name = f" {settings['molecule_name']}" if settings["molecule_name"] else ""
    lines = [f"molecule{name} {{"]
    lines += [f"  {line}".rstrip() for line in _geometry_lines(settings)]
    lines += ["}", ""]

    blocks: dict[str, list[str]] = {}
    for module, option, value in _options(settings):
        blocks.setdefault(module, []).append(f"  {option} {_psithon_value(value)}")
    for module, entries in blocks.items():
        lines += [f"set {module} {{" if module else "set {", *entries, "}", ""]

    for driver, method, arguments in _calls(settings):
        written = ", ".join([repr(method), *(f"{key}={value!r}" for key, value in arguments)])
        lines.append(f"{driver}({written})")
    return "\n".join(lines) + "\n"


def _psiapi(settings: Mapping[str, Any]) -> str:
    lines = [
        "import psi4",
        "",
        'psi4.set_output_file("output.dat", False)',
        "",
    ]
    geometry = "\n".join(_geometry_lines(settings))
    name = f', name="{settings["molecule_name"]}"' if settings["molecule_name"] else ""
    lines += [f'psi4.geometry("""\n{geometry}\n"""{name})', ""]

    options = _options(settings)
    if options:
        lines.append("psi4.set_options({")
        for module, option, value in options:
            key = f"{module}__{option}" if module else option
            lines.append(f"    {key!r}: {_python_value(value)!r},")
        lines += ["})", ""]

    for driver, method, arguments in _calls(settings):
        written = ", ".join([repr(method), *(f"{key}={value!r}" for key, value in arguments)])
        lines.append(f"psi4.{driver}({written})")
    return "\n".join(lines) + "\n"


def from_boxes(payload: dict[str, Any]) -> str:
    settings = _validated(payload)
    return _psiapi(settings) if settings["input_format"] == PSIAPI else _psithon(settings)


# --- Reading --------------------------------------------------------------------


def _memory_mib(value: str | int | float) -> str:
    if isinstance(value, (int, float)):
        number, unit = Decimal(str(value)), "b"
    else:
        match = MEMORY.fullmatch(value.strip())
        if match is None:
            raise ToolInputError(NOT_A_FORM)
        number, unit = Decimal(match[1]), (match[2] or "b").lower()
    return format((number * MEMORY_UNITS[unit] / MIB).normalize(), "f")


def _store_option(boxes: dict[str, Any], extras: list[str], module: str, option: str, value: str):
    module, option = module.lower(), option.lower()
    field = OPTION_FIELDS.get((module, option))
    if field is not None:
        candidate = value.lower() if FIELDS[field].kind == "select" else value
        allowed = {item.value for item in FIELDS[field].options}
        if FIELDS[field].kind != "select" or candidate in allowed:
            boxes[field] = candidate
            return
    extras.append(f"{module}__{option} {value}" if module else f"{option} {value}")


def _store_calls(boxes: dict[str, Any], calls: list[tuple[str, str, dict[str, Any]]]):
    drivers = tuple(driver for driver, _, _ in calls)
    calculation = next((name for name, steps in DRIVERS.items() if steps == drivers), None)
    if calculation is None or len({method for _, method, _ in calls}) != 1:
        raise ToolInputError(NOT_A_FORM)
    boxes["calculation"] = calculation
    boxes["method"] = calls[0][1]
    for driver, _, arguments in calls:
        if "dertype" in arguments:
            boxes["frequency_derivative"] = str(arguments["dertype"])
        if "properties" in arguments:
            boxes["properties"] = ", ".join(arguments["properties"])


def _store_geometry(boxes: dict[str, Any], lines: list[str]):
    lines = [line.strip() for line in lines if line.strip()]
    if lines and re.fullmatch(r"[+-]?[0-9]+\s+[0-9]+", lines[0]):
        boxes["charge"], boxes["multiplicity"] = lines.pop(0).split()
    else:
        boxes["charge"] = boxes["multiplicity"] = ""
    boxes["coordinates"] = "\n".join(lines)


def _driver_arguments(arguments: str, molecule: str) -> dict[str, Any]:
    """Keyword arguments of a Psithon driver call, as Python values."""

    try:
        call = ast.parse(f"f({arguments})", mode="eval").body
        values = {keyword.arg: keyword.value for keyword in call.keywords}
        if call.args or None in values:
            raise ValueError
        found = {}
        for key, node in values.items():
            if key == "molecule" and isinstance(node, ast.Name) and node.id == molecule:
                continue
            found[key] = ast.literal_eval(node)
    except (SyntaxError, ValueError):
        raise ToolInputError(NOT_A_FORM) from None
    return _known_arguments(found)


def _known_arguments(arguments: dict[str, Any]) -> dict[str, Any]:
    arguments.pop("return_wfn", None)
    if set(arguments) - {"dertype", "properties"}:
        raise ToolInputError(NOT_A_FORM)
    if "dertype" in arguments and arguments["dertype"] not in (0, 1, 2):
        raise ToolInputError(NOT_A_FORM)
    properties = arguments.get("properties", [])
    if not isinstance(properties, list) or not all(isinstance(item, str) for item in properties):
        raise ToolInputError(NOT_A_FORM)
    return arguments


def _read_psithon(document: str) -> dict[str, Any]:
    boxes: dict[str, Any] = {"input_format": PSITHON, "molecule_name": ""}
    extras: list[str] = []
    calls = []
    lines = iter(
        (number, line.partition("#")[0].strip())
        for number, line in enumerate(document.splitlines(), start=1)
    )

    def block():
        for _, content in lines:
            if content == "}":
                return
            yield content
        raise ToolInputError(NOT_A_FORM)

    driver_call = re.compile(
        rf"(?:{IDENT}(?:\s*,\s*{IDENT})*\s*=\s*)?({IDENT})\s*\(\s*(['\"])([^'\"]+)\2\s*(?:,(.*))?\)"
    )
    for _, line in lines:
        if not line:
            continue
        if call := driver_call.fullmatch(line):
            driver = DRIVER_ALIASES.get(call[1].lower(), call[1].lower())
            if driver not in {"energy", "gradient", "properties", "optimize", "frequency", "hessian"}:
                raise ToolInputError(NOT_A_FORM)
            calls.append((driver, call[3], _driver_arguments(call[4] or "", boxes["molecule_name"])))
        elif calls:
            raise ToolInputError(NOT_A_FORM)
        elif match := re.fullmatch(rf"molecule(?:\s+({IDENT}))?\s*\{{", line):
            if "coordinates" in boxes:
                raise ToolInputError(NOT_A_FORM)
            boxes["molecule_name"] = match[1] or ""
            _store_geometry(boxes, list(block()))
        elif match := re.fullmatch(rf"set(?:\s+({IDENT}))?\s*\{{", line):
            for entry in block():
                option = OPTION_LINE.fullmatch(entry)
                if option is None:
                    raise ToolInputError(NOT_A_FORM)
                _store_option(boxes, extras, match[1] or "", option[1], option[2])
        elif match := re.fullmatch(rf"set\s+(?:({IDENT})\s+)?({IDENT})\s+([^{{]+)", line):
            # "set scf maxiter 50" and "set basis cc-pvdz" read the same way.
            module, option, value = match[1] or "", match[2], match[3].strip()
            if module and not re.fullmatch(r"\S+", value):
                module, option, value = "", module, f"{option} {value}"
            _store_option(boxes, extras, module, option, value)
        elif match := re.fullmatch(r"memory\s+(.+)", line, re.IGNORECASE):
            boxes["memory_mib"] = _memory_mib(match[1])
        elif match := re.fullmatch(r"set_num_threads\(\s*([0-9]+)\s*\)", line):
            boxes["threads"] = match[1]
        else:
            raise ToolInputError(NOT_A_FORM)
    if "coordinates" not in boxes or not calls:
        raise ToolInputError(NOT_A_FORM)
    _store_calls(boxes, calls)
    boxes["extra_options"] = "\n".join(extras)
    return boxes


def _psi4_call(node: ast.AST) -> tuple[str, ast.Call] | None:
    if (
        isinstance(node, ast.Call)
        and isinstance(node.func, ast.Attribute)
        and isinstance(node.func.value, ast.Name)
        and node.func.value.id == "psi4"
    ):
        return node.func.attr, node
    return None


def _read_psiapi(document: str) -> dict[str, Any]:
    try:
        module = ast.parse(document)
    except SyntaxError:
        raise ToolInputError(NOT_A_FORM) from None
    boxes: dict[str, Any] = {"input_format": PSIAPI, "molecule_name": ""}
    extras: list[str] = []
    calls = []
    molecule = ""
    for statement in module.body:
        if isinstance(statement, ast.Import) and [alias.name for alias in statement.names] == ["psi4"]:
            continue
        targets = []
        if isinstance(statement, ast.Assign):
            targets, value = statement.targets, statement.value
        elif isinstance(statement, ast.Expr):
            value = statement.value
        else:
            raise ToolInputError(NOT_A_FORM)
        found = _psi4_call(value)
        if found is None:
            raise ToolInputError(NOT_A_FORM)
        name, call = found
        try:
            arguments = [ast.literal_eval(argument) for argument in call.args]
            keywords = {
                keyword.arg: keyword.value for keyword in call.keywords if keyword.arg
            }
        except ValueError:
            raise ToolInputError(NOT_A_FORM) from None
        driver = DRIVER_ALIASES.get(name, name)
        if driver in {"energy", "gradient", "properties", "optimize", "frequency", "hessian"}:
            if len(arguments) != 1 or not isinstance(arguments[0], str):
                raise ToolInputError(NOT_A_FORM)
            given = keywords.pop("molecule", None)
            if given is not None and not (isinstance(given, ast.Name) and given.id == molecule):
                raise ToolInputError(NOT_A_FORM)
            try:
                literal = {key: ast.literal_eval(node) for key, node in keywords.items()}
            except ValueError:
                raise ToolInputError(NOT_A_FORM) from None
            calls.append((driver, arguments[0], _known_arguments(literal)))
            continue
        if calls or targets and name != "geometry":
            raise ToolInputError(NOT_A_FORM)
        if name == "set_output_file":
            continue
        if name == "geometry":
            if "coordinates" in boxes or len(arguments) != 1 or set(keywords) - {"name"}:
                raise ToolInputError(NOT_A_FORM)
            if len(targets) > 1 or targets and not isinstance(targets[0], ast.Name):
                raise ToolInputError(NOT_A_FORM)
            molecule = targets[0].id if targets else ""
            if "name" in keywords:
                boxes["molecule_name"] = ast.literal_eval(keywords["name"])
            _store_geometry(boxes, str(arguments[0]).splitlines())
        elif name == "set_options" and len(arguments) == 1 and isinstance(arguments[0], dict):
            for key, value in arguments[0].items():
                module, _, option = str(key).rpartition("__")
                if isinstance(value, bool):
                    value = str(value).lower()
                elif isinstance(value, float):
                    value = format(value, "g")
                _store_option(boxes, extras, module, option, str(value))
        elif name == "set_memory" and len(arguments) == 1:
            boxes["memory_mib"] = _memory_mib(arguments[0])
        elif name == "set_num_threads" and len(arguments) == 1 and isinstance(arguments[0], int):
            boxes["threads"] = str(arguments[0])
        else:
            raise ToolInputError(NOT_A_FORM)
    if "coordinates" not in boxes or not calls:
        raise ToolInputError(NOT_A_FORM)
    _store_calls(boxes, calls)
    boxes["extra_options"] = "\n".join(extras)
    return boxes


def to_boxes(document: str, values: Mapping[str, Any] | None = None) -> dict[str, Any]:
    """The form fields for a raw input, when the form can express all of it."""

    if len(document) > MAX_INPUT:
        raise ToolInputError(f"Psi4 input is too long (maximum {MAX_INPUT} characters).")
    language = input_format(values or {})
    boxes = _read_psiapi(document) if language == PSIAPI else _read_psithon(document)
    # Options the input leaves unset stay unset.
    for name in FIELD_OPTIONS:
        boxes.setdefault(name, "" if name == "basis" else FIELDS[name].default)
    boxes.setdefault("frequency_derivative", FIELDS["frequency_derivative"].default)
    boxes.setdefault("properties", "")
    settings = _validated({**(values or {}), **boxes})
    for name in ("e_convergence", "d_convergence"):
        # The field holds the tolerance, not Psi4's integer shorthand.
        if settings[name] is not None:
            boxes[name] = format(settings[name], "g")
    return {name: "" if value is None else str(value) for name, value in boxes.items()} | {
        "threads": str(settings["threads"]),
        "memory_mib": format(settings["memory_mib"], "f"),
    }
