"""The adapter coordinator's bindings to its Rust host.

Catalog data, bundled inputs, process execution and auditing are provided by the
same bio_tools library linked into the desktop application. This module performs
only the Python/Rust serialization, so no published native wheel is required.
"""
from __future__ import annotations

from dataclasses import fields, is_dataclass
from enum import Enum
from functools import lru_cache
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace


class RunError(RuntimeError):
    pass


class LaunchType(Enum):
    PythonLib = 'PythonLib'
    PythonBasedApp = 'PythonBasedApp'
    CondaBasedApp = 'CondaBasedApp'
    Executable = 'Executable'


Process = SimpleNamespace


def _rpc(op, **arguments):
    with tempfile.TemporaryDirectory(prefix='bio-tools-rpc-') as temporary:
        request = Path(temporary) / 'request.json'
        response = Path(temporary) / 'response.json'
        request.write_text(json.dumps({'op': op, **arguments}, default=os.fspath), encoding='utf-8')
        completed = subprocess.run(
            [os.environ['BIO_TOOLS_ADAPTER_HOST'], '--bio-tools-adapter-rpc', str(request), str(response)],
            stdin=subprocess.DEVNULL,
            creationflags=subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0,
        )
        if completed.returncode or not response.is_file():
            raise RuntimeError('The bio_tools host stopped without returning a response.')
        envelope = json.loads(response.read_text(encoding='utf-8'))
        if 'error' in envelope:
            raise ValueError(envelope['error'])
        return envelope['result']


@lru_cache(maxsize=None)
def _catalog(slug):
    return _rpc('catalog', slug=slug)


def catalog_fields(slug, *, field_type, option_type, dynamic_options=None):
    allowed = {field.name for field in fields(field_type)} if is_dataclass(field_type) else None
    result = []
    for field in _catalog(slug)['fields']:
        values = {key: value for key, value in field.items() if allowed is None or key in allowed}
        options = (dynamic_options or {}).get(field['name'])
        values['options'] = [option_type(*option) for option in options] if options is not None else [option_type(**option) for option in field.get('options', [])]
        result.append(field_type(**values))
    return result


def catalog_tasks(slug, *, option_type):
    return [option_type(**option) for option in _catalog(slug).get('tasks', [])]


def catalog_spec(slug, *, fields=None, tasks=None):
    return SimpleNamespace(slug=slug, fields=fields or [], tasks=tasks or [])


def catalog_process(identifier, module):
    return Process(id=identifier, module=module, spec=module.SPEC)


def catalog_preset(slug, preset_id, *, overrides=None, workdir=None):
    return _rpc('preset', slug=slug, preset=preset_id, overrides=overrides or {}, workdir=workdir)


def catalog_input_text(slug, value):
    return _rpc('input_text', slug=slug, value=value)


def catalog_asset(slug, name):
    return _rpc('asset', slug=slug, name=name)


def chai1_example_msas(root):
    return _rpc('chai1_example_msas', root=root)


class CommandSpec:
    def __init__(self, command, **options):
        self.arguments = {'command': [os.fspath(argument) for argument in command], **options}

    def run(self):
        try:
            return SimpleNamespace(**_rpc('run', **self.arguments))
        except ValueError as error:
            raise RunError(str(error)) from error


def install():
    sys.modules['bio_tools'] = sys.modules[__name__]
