"""Run in ESMC's isolated environment; export portable arrays and score tables."""

from __future__ import annotations

import argparse
import csv
import json
from importlib.metadata import version
from pathlib import Path


def infer(request: dict, output_dir: Path) -> dict:
    import numpy as np
    import torch
    from esm.models.esmc import EsmcForMaskedLM, EsmcTokenizer

    device = request["device"]
    if device == "auto":
        device = "cuda" if torch.cuda.is_available() else "cpu"
    if device == "cuda" and not torch.cuda.is_available():
        raise RuntimeError("CUDA was requested, but PyTorch cannot access a CUDA GPU.")
    precision = request["precision"]
    if precision == "bf16" and device == "cuda" and not torch.cuda.is_bf16_supported():
        raise RuntimeError("This CUDA device does not support BF16; choose FP32.")
    dtype = {"default": None, "fp32": torch.float32, "bf16": torch.bfloat16}[precision]
    print(f"Loading {request['model']} on {device}.", flush=True)
    model = EsmcForMaskedLM.from_pretrained(
        request["model"],
        device=device,
        dtype=dtype,
        attn_implementation="sdpa",
    ).eval()
    tokenizer = EsmcTokenizer()
    output_dir.mkdir(parents=True, exist_ok=True)
    records = request["records"]
    canonical = "ACDEFGHIKLMNPQRSTVWY"
    aa_ids = tokenizer.convert_tokens_to_ids(list(canonical))
    layer = request["hidden_layer"]
    if layer > model.config.num_hidden_layers:
        raise ValueError(
            "The requested hidden layer exceeds this checkpoint's layer count."
        )

    def inputs_for(sequences):
        encoded = tokenizer(sequences, padding=True, return_tensors="pt")
        return {key: value.to(device) for key, value in encoded.items()}

    def array(tensor):
        # NumPy does not support torch.bfloat16; all exported floating arrays use FP32.
        return tensor.detach().float().cpu().numpy()

    summaries = []
    batch_size = request["batch_size"]
    with (output_dir / "sequences.fasta").open("w", encoding="utf-8") as fasta:
        for record in records:
            fasta.write(f">{record['id']}\n{record['sequence']}\n")
    with (
        torch.inference_mode(),
        (output_dir / "residue_predictions.csv").open(
            "w", newline="", encoding="utf-8"
        ) as handle,
    ):
        writer = csv.writer(handle)
        writer.writerow(
            [
                "sequence_id",
                "position",
                "residue",
                "top_amino_acid",
                "top_probability",
                "observed_log_probability",
                "canonical_entropy_nats",
            ]
        )
        for start in range(0, len(records), batch_size):
            batch = records[start : start + batch_size]
            inputs = inputs_for([record["sequence"] for record in batch])
            output = model(**inputs, output_hidden_states=layer >= 0)
            for offset, record in enumerate(batch):
                length = len(record["sequence"])
                # Every input contains exactly BOS + residues + EOS, followed by padding.
                span = slice(1, length + 1)
                logits = output.logits[offset, span].float()
                log_probs = logits.log_softmax(dim=-1)
                aa_log_probs = logits[:, aa_ids].log_softmax(dim=-1)
                aa_probs = aa_log_probs.exp()
                top = aa_probs.argmax(dim=-1)
                entropy = -(aa_probs * aa_log_probs).sum(dim=-1)
                observed = log_probs.gather(
                    1, inputs["input_ids"][offset, span, None]
                ).squeeze(-1)
                for pos, residue in enumerate(record["sequence"]):
                    best = int(top[pos])
                    writer.writerow(
                        [
                            record["id"],
                            pos + 1,
                            residue,
                            canonical[best],
                            float(aa_probs[pos, best]),
                            float(observed[pos]),
                            float(entropy[pos]),
                        ]
                    )
                arrays = {"token_ids": inputs["input_ids"][offset, span].cpu().numpy()}
                if request["save_embeddings"]:
                    embeddings = output.last_hidden_state[offset, span]
                    arrays["embeddings"] = array(embeddings)
                    arrays["mean_embedding"] = array(embeddings.float().mean(dim=0))
                if request["save_logits"]:
                    arrays["logits"] = array(logits)
                    arrays["probabilities"] = array(log_probs.exp())
                if layer >= 0:
                    arrays["hidden_state"] = array(
                        output.hidden_states[layer, offset, span]
                    )
                filename = f"sequence_{start + offset + 1:03d}.npz"
                np.savez_compressed(output_dir / filename, **arrays)
                summaries.append(
                    {
                        "id": record["id"],
                        "length": length,
                        "array_file": filename,
                        "embedding_dimension": model.config.hidden_size,
                        "mean_unmasked_log_probability": float(observed.mean()),
                        "mean_canonical_entropy_nats": float(entropy.mean()),
                        "arrays": {
                            key: list(value.shape) for key, value in arrays.items()
                        },
                    }
                )
            del output, inputs
            print(
                f"Analyzed {min(start + batch_size, len(records))}/{len(records)} proteins.",
                flush=True,
            )

        if request["mutations"]:
            sequence = records[0]["sequence"]
            inputs = inputs_for([sequence])
            # Mask once per distinct position; multiple alternatives share that distribution.
            by_position = {}
            for mutation in request["mutations"]:
                by_position.setdefault(mutation["position"], []).append(mutation)
            with (output_dir / "mutation_scores.csv").open(
                "w", newline="", encoding="utf-8"
            ) as scores:
                score_writer = csv.writer(scores)
                score_writer.writerow(
                    [
                        "sequence_id",
                        "mutation",
                        "position",
                        "wild_type",
                        "mutant",
                        "masked_log_odds",
                    ]
                )
                for position, mutations in by_position.items():
                    masked = {key: value.clone() for key, value in inputs.items()}
                    masked["input_ids"][0, position] = tokenizer.mask_token_id
                    prediction = model(**masked)
                    logits = prediction.logits[0, position].float()
                    for mutation in mutations:
                        wt_id, mutant_id = tokenizer.convert_tokens_to_ids(
                            [mutation["wild_type"], mutation["mutant"]]
                        )
                        score = float(logits[mutant_id] - logits[wt_id])
                        score_writer.writerow(
                            [
                                records[0]["id"],
                                mutation["mutation"],
                                position,
                                mutation["wild_type"],
                                mutation["mutant"],
                                score,
                            ]
                        )
                    del prediction

    with (output_dir / "sequence_summary.csv").open(
        "w", newline="", encoding="utf-8"
    ) as handle:
        writer = csv.DictWriter(
            handle,
            fieldnames=[key for key in summaries[0] if key != "arrays"],
            extrasaction="ignore",
        )
        writer.writeheader()
        writer.writerows(summaries)
    vocabulary_size = model.config.vocab_size
    vocabulary = {
        str(index): tokenizer.convert_ids_to_tokens(index)
        for index in range(vocabulary_size)
    }
    summary = {
        "model": request["model"],
        "esm_version": version("esm"),
        "torch_version": torch.__version__,
        "device": device,
        "dtype": str(next(model.parameters()).dtype),
        "batch_size": batch_size,
        "hidden_layer": layer,
        "sequences": summaries,
        "mutation_count": len(request["mutations"]),
        "vocabulary": vocabulary,
        "notes": [
            "Residue positions are one-based. Arrays exclude BOS, EOS and padding. Each FASTA entry is processed independently.",
            "Embeddings are final post-layer-normalization representations; pooling is the arithmetic mean over residues.",
            "Hidden-state indices follow upstream: 0 is the first block input; the last index is final post-layer-normalization output.",
            "Top probabilities and entropy are normalized over 20 canonical amino acids; observed log probabilities and NPZ probabilities use the full model vocabulary. Unused vocabulary slots are null in the vocabulary map.",
            "Residue predictions use unmasked inputs and are not pseudo-likelihood or calibrated fitness scores.",
            "Mutation scores are log P(mutant|masked context) minus log P(wild type|masked context), each substitution scored independently. Positive means the model favors the mutant; it does not establish improved function or stability.",
        ],
    }
    (output_dir / "summary.json").write_text(
        json.dumps(summary, indent=2, allow_nan=False), encoding="utf-8"
    )
    return summary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    args = parser.parse_args()
    request = json.loads(args.input.read_text(encoding="utf-8"))
    infer(request, args.output_dir)


if __name__ == "__main__":
    main()
