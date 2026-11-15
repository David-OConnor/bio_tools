"""Select the fastest usable accelerator in OpenFold3's PyTorch environment."""

import json
import os
import platform
import sys
from pathlib import Path

import torch


def probe(requested: str) -> dict[str, str | bool]:
    if requested == "auto":
        candidates = ("rocm" if torch.version.hip else "cuda", "mps", "cpu")
    else:
        candidates = (requested,)

    for backend in candidates:
        if backend in {"cuda", "rocm"}:
            if (backend == "rocm") != bool(torch.version.hip) or not torch.cuda.is_available():
                continue
            device = "cuda"  # PyTorch uses the CUDA device API for ROCm too.
        elif backend == "mps":
            if platform.machine() != "arm64" or not torch.backends.mps.is_available():
                continue
            device = "mps"
        elif backend == "cpu":
            device = "cpu"
        else:
            raise ValueError("OPENFOLD3_BACKEND must be auto, cpu, cuda, rocm, or mps.")

        try:
            torch.ones(1, device=device).item()
        except (AssertionError, RuntimeError):
            if requested != "auto" or device == "cpu":
                raise
            continue

        # Use standard PyTorch kernels on ROCm and pre-Ampere NVIDIA cards.
        triton = backend == "cuda" and torch.cuda.get_device_capability()[0] >= 8
        return {"backend": backend, "device": device, "triton": triton}

    raise RuntimeError(f"OpenFold3 could not use the requested {requested} backend.")


if __name__ == "__main__":
    requested = sys.argv[1] if len(sys.argv) > 1 else os.getenv("OPENFOLD3_BACKEND")
    if not requested:
        setting = Path(sys.prefix) / "openfold3-backend"
        requested = setting.read_text(encoding="utf-8") if setting.is_file() else "auto"
    print(json.dumps(probe(requested.strip().lower())))
