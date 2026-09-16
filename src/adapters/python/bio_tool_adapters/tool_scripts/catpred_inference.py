"""Run in CatPred's isolated environment; predict kcat, Km or Ki with uncertainty.

Drives `catpred.inference`, the same pipeline `demo_run.py` and the upstream
`/predict` API use, so a run here and a run of CatPred's own demo differ only in
where the input CSV came from: the ensemble is read with `uncertainty_method`
"mve", and the postprocessing that turns log10 predictions into a linear value
with total/aleatoric/epistemic standard deviations is upstream's own.
"""

from __future__ import annotations

import argparse
import csv
import json
import os
from pathlib import Path

# The unit each parameter is predicted in, and the column upstream writes the
# log10 prediction into. Mirrors `catpred.inference.service._TARGET_COLUMNS`.
UNITS = {"kcat": "s^(-1)", "km": "mM", "ki": "mM"}
PREDICTION_COLUMNS = ("Prediction_log10", "SD_total", "SD_aleatoric", "SD_epistemic")


def resolve_device(requested: str) -> bool:
    """Whether to run on the GPU, refusing a CUDA request nothing can serve."""

    import torch

    available = torch.cuda.is_available()
    if requested == "cuda" and not available:
        raise RuntimeError("CUDA was requested, but PyTorch cannot access a CUDA GPU.")
    return requested == "cuda" or (requested == "auto" and available)


def write_input_csv(rows: list[dict], path: Path) -> list[str]:
    """CatPred's own input CSV: SMILES, sequence and pdbpath, plus any label.

    Upstream requires those three columns and carries every other column
    through to the output, which is how the substrate names in its demo CSVs
    survive into the predictions.
    """

    columns = ["SMILES", "sequence", "pdbpath"]
    for row in rows:
        for key in row:
            if key not in columns:
                columns.append(key)
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=columns, extrasaction="ignore")
        writer.writeheader()
        writer.writerows(rows)
    return columns


def predict(request: dict, work_dir: Path, output_dir: Path) -> dict:
    import torch

    use_gpu = resolve_device(request["device"])
    parameter = request["parameter"]
    checkpoint_dir = Path(request["checkpoint_dir"]).resolve()

    # Upstream refuses to torch.load() a checkpoint outside an allowlist that
    # otherwise covers only the working directory and its parent, and this
    # run's working directory is a temporary one. The checkpoints and the ESM-2
    # embedding cache are the two artifacts it loads, so those roots are named
    # here rather than the check being turned off. This is CatPred's own
    # defence against arbitrary pickle deserialization.
    roots = [str(checkpoint_dir)]
    cache = os.environ.get("CATPRED_CACHE_PATH")
    if cache:
        roots.append(str(Path(cache).expanduser()))
    existing = os.environ.get("CATPRED_TRUSTED_DESERIALIZATION_ROOTS")
    if existing:
        roots.append(existing)
    os.environ["CATPRED_TRUSTED_DESERIALIZATION_ROOTS"] = os.pathsep.join(roots)

    from catpred.inference import PredictionRequest, run_inprocess_prediction_pipeline

    work_dir.mkdir(parents=True, exist_ok=True)
    output_dir.mkdir(parents=True, exist_ok=True)
    input_csv = work_dir / "input.csv"
    write_input_csv(request["rows"], input_csv)

    print(
        f"Predicting {parameter} for {len(request['rows'])} row(s) on "
        f"{'GPU' if use_gpu else 'CPU'} using {checkpoint_dir}.",
        flush=True,
    )
    produced = run_inprocess_prediction_pipeline(
        request=PredictionRequest(
            parameter=parameter,
            input_file=str(input_csv),
            checkpoint_dir=str(checkpoint_dir),
            use_gpu=use_gpu,
            repo_root=str(work_dir),
        ),
        results_dir=str(work_dir / "results"),
    )

    # Upstream names the file after the input it came from; the run's own name
    # for it says what it holds instead.
    predictions = output_dir / f"{parameter}_predictions.csv"
    predictions.write_text(
        Path(produced).read_text(encoding="utf-8"), encoding="utf-8"
    )

    unit = UNITS[parameter]
    with predictions.open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    summary_rows = []
    for row in rows:
        summary_rows.append(
            {
                "id": row.get("pdbpath", ""),
                "substrate": row.get("Substrate", ""),
                "smiles": row.get("SMILES", ""),
                "sequence_length": len(row.get("sequence", "")),
                "prediction": _number(row.get(f"Prediction_({unit})")),
                **{
                    column.lower(): _number(row.get(column))
                    for column in PREDICTION_COLUMNS
                },
            }
        )

    summary = {
        "parameter": parameter,
        "unit": unit,
        "device": "cuda" if use_gpu else "cpu",
        "torch_version": torch.__version__,
        "checkpoint_dir": str(checkpoint_dir),
        "checkpoints": sorted(
            path.relative_to(checkpoint_dir).as_posix()
            for path in checkpoint_dir.rglob("model.pt")
        ),
        "row_count": len(summary_rows),
        "predictions": summary_rows,
        "notes": [
            "Predictions are made in log10 space and reported both ways: "
            f"Prediction_({unit}) is 10 ** Prediction_log10.",
            "SD_total is the ensemble's mean-variance-estimation standard deviation in "
            "log10 units, split into SD_aleatoric (data noise) and SD_epistemic "
            "(disagreement between the ensemble's models).",
            "A prediction one standard deviation wide in log10 spans a factor of "
            "10 ** SD_total either side of the value.",
            "kcat is predicted for the reaction's full substrate set, Km for the one "
            "substrate given, and Ki for the inhibitor.",
            "Sequences are featurized with ESM-2, which truncates beyond 2046 residues.",
        ],
    }
    (output_dir / "summary.json").write_text(
        json.dumps(summary, indent=2, allow_nan=False), encoding="utf-8"
    )
    return summary


def _number(value) -> float | None:
    try:
        number = float(value)
    except (TypeError, ValueError):
        return None
    return number if number == number and abs(number) != float("inf") else None


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--work-dir", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    args = parser.parse_args()
    request = json.loads(args.input.read_text(encoding="utf-8"))
    predict(request, args.work_dir, args.output_dir)


if __name__ == "__main__":
    main()
