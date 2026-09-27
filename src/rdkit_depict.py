"""Read one JSON depiction request from stdin; write SVG to stdout."""

import json
import sys

from rdkit import Chem
from rdkit.Chem import rdDepictor
from rdkit.Chem.Draw import rdMolDraw2D


def main():
    request = json.load(sys.stdin)
    molecule = Chem.MolFromSmiles(request["smiles"])
    if molecule is None:
        raise ValueError("RDKit could not parse the supplied SMILES")

    rdDepictor.Compute2DCoords(molecule)
    drawer = rdMolDraw2D.MolDraw2DSVG(request["width"], request["height"])
    rdMolDraw2D.PrepareAndDrawMolecule(drawer, molecule)
    drawer.FinishDrawing()
    sys.stdout.buffer.write(drawer.GetDrawingText().encode("utf-8"))


if __name__ == "__main__":
    main()
