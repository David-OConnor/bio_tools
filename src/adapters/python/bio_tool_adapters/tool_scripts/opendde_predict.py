"""Run the `opendde` CLI with triangle-attention chunks sized to the GPU.

OpenDDE picks its chunk size from the token count alone
(`infer_setting.chunk_size_thresholds`) and leaves attention unchunked up to
1,024 tokens. That is safe with its cuEquivariance kernels, which never hold
the attention logits whole, but those run only on Linux x86_64. Everywhere
else, including Windows, it falls back to torch kernels, whose unchunked fp32
logits over 12 heads are 48 * N^3 bytes: 43 GiB for the 990 structural tokens
a 510-residue protein expands to. On a 16 GB card that is an out-of-memory
error after the whole trunk has already run.

With torch kernels, this keeps OpenDDE's own choice when it fits, and
otherwise lowers it to the largest power of two whose attention logits fit in
the memory that is free. Arguments are passed to the `opendde` CLI unchanged.
"""

from __future__ import annotations

import logging
import math

import torch
from opendde.model.opendde import OpenDDE
from opendde.model.triangular.triangular import TriangleAttention
from runner.cli import opendde_cli

logger = logging.getLogger(__name__)

_FP32_BYTES = 4


def _free_bytes(device: torch.device) -> int:
    free, _total = torch.cuda.mem_get_info(device)
    # What PyTorch's allocator holds but does not use is free to the next tensor.
    return (
        free + torch.cuda.memory_reserved(device) - torch.cuda.memory_allocated(device)
    )


def _chunk_cap(model: OpenDDE, n_token: int) -> int | None:
    """The largest chunk whose triangle attention fits in free GPU memory.

    None when the whole attention fits, when the model is not on a GPU, or
    when triangle attention runs on cuEquivariance's kernel.
    """

    device = next(model.parameters()).device
    if device.type != "cuda" or model.configs.triangle_attention != "torch":
        return None
    attention = [
        module.mha for module in model.modules() if isinstance(module, TriangleAttention)
    ]
    heads = max(mha.no_heads for mha in attention)
    c_z = max(mha.c_q for mha in attention)
    pairs = n_token * n_token
    # The pair representation, its normalized copy and the chunked output stay
    # whole while the chunks run, next to other pair-sized tensors.
    whole = 4 * pairs * c_z * _FP32_BYTES
    # Each row of a chunk holds its attention logits and their softmax.
    per_row = 2 * heads * pairs * _FP32_BYTES
    # Half of what is left, for everything this does not count.
    rows = (_free_bytes(device) - whole) // 2 // per_row
    if rows >= n_token:
        return None
    return 2 ** int(math.log2(rows)) if rows >= 1 else 1


_upstream_chunk_size = OpenDDE._get_dynamic_chunk_size


def _get_dynamic_chunk_size(self: OpenDDE, N_token: int) -> int | None:
    chunk = _upstream_chunk_size(self, N_token)
    cap = _chunk_cap(self, N_token)
    if cap is None or (chunk is not None and chunk <= cap):
        return chunk
    logger.info(
        "Chunk size for %d tokens lowered from %s to %d to fit in GPU memory.",
        N_token,
        chunk,
        cap,
    )
    return cap


OpenDDE._get_dynamic_chunk_size = _get_dynamic_chunk_size

if __name__ == "__main__":
    opendde_cli()
