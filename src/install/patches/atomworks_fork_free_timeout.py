"""Give atomworks a conformer-generation time limit that does not need `fork`.

RFdiffusion3 builds its atom-level reference features through atomworks, which
generates an RDKit conformer per ligand or nucleic-acid residue under a time
limit. atomworks applies that limit by running the work in a `fork`ed child
(`strategy="subprocess"`, the default) or under `SIGALRM`
(`strategy="signal"`); Windows offers neither, so the decorator raises before
the wrapped function is ever called:

    ValueError: Transforms failed at stage `CreateDesignReferenceFeatures`:
    cannot find context for 'fork'

Protein-only designs never reach it -- RFD3 asks for conformers of the
non-protein residues alone -- so the failure lands only on the inputs that
carry a ligand, a nucleic acid, or another non-protein component.

Nothing upstream exposes the strategy or the limit as a setting:
`rfd3.transforms.design_transforms.CreateDesignReferenceFeatures` hard-codes
`conformer_generation_timeout=2.0` and passes no strategy, and
`rfd3.transforms.pipelines` constructs that transform with named arguments
alone, so no Hydra override reaches it. atomworks is the only place to act.

The block appended below restores upstream's own `timeout=None` behaviour --
call the function directly -- on any platform with no `fork`. The limit
guards against a pathological RDKit embedding rather than deciding a result:
every call site catches the `TimeoutError` and falls back to zeroed reference
coordinates, so a design that would have lost its conformer keeps it, at the
cost of no upper bound on how long that one embedding may take.

Sources:
  atomworks               https://github.com/RosettaCommons/atomworks
  the timeout helpers     https://github.com/RosettaCommons/atomworks/blob/production/src/atomworks/ml/utils/timer.py
  the conformer callers   https://github.com/RosettaCommons/atomworks/blob/production/src/atomworks/ml/transforms/rdkit_utils.py
  the RFD3 transform      https://github.com/RosettaCommons/foundry/blob/production/models/rfd3/src/rfd3/transforms/design_transforms.py

Run with the RFdiffusion3 environment's own interpreter. It edits the
installed `atomworks/ml/utils/timer.py` in place, and is safe to rerun: the
marker below is what tells it the environment is already patched.
"""

import importlib
import pathlib

from atomworks.ml.utils import timer

MARKER = "# --- bio_tools: a timeout for platforms without fork ---"

BLOCK = (
    '''

'''
    + MARKER
    + '''
# `timeout_using_subprocess` above asks multiprocessing for the "fork" start
# method and `timeout_using_signal` asks for SIGALRM. Where the platform has
# neither -- Windows -- both raise before the wrapped function runs, so fall
# back to what this module already does for `timeout=None` and call the
# function directly. "spawn" is not the alternative it looks like: it reimports
# RDKit and atomworks in the child, which alone outlasts the 2 second limit
# RFdiffusion3 asks for, and every conformer would degrade to the zeroed
# fallback.
import os as _os  # noqa: E402

if not hasattr(_os, "fork"):

    def timeout(  # noqa: F811
        timeout: float | int | None = None,
        strategy: Literal["signal", "subprocess"] = "subprocess",
    ) -> Callable:
        """Call the wrapped function directly: nothing here can interrupt it."""

        return do_nothing()
'''
)


def _probe() -> str:
    return "ok"


def main() -> None:
    path = pathlib.Path(timer.__file__)
    source = path.read_text(encoding="utf-8")
    if MARKER not in source:
        path.write_text(source + BLOCK, encoding="utf-8")
    # A decorated call has to return normally on a platform with no `fork`.
    # Before the guard it raised `ValueError: cannot find context for 'fork'`.
    patched = importlib.reload(timer)
    assert patched.timeout(timeout=1.0)(_probe)() == "ok", (
        "atomworks' timeout decorator did not run its function after patching"
    )


if __name__ == "__main__":
    main()
