use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::catalog::{CatalogEntry, DataType, Identity},
};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Uninstalled {
        slug: "rdkit",
        name: "RDKit",
    },
    categories: &[ToolCategory::Cheminformatics],
    launch_type: LaunchType::PythonLib,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Cheap,
    // One table per run: a row per molecule, or per reaction, with whichever
    // properties and bond edits were asked for.
    primary_output: Some(DataType::Csv),
    // Nothing in this catalog produces a molecule: the design and prediction
    // tools hand on sequences and coordinates, and RDKit's input is SMILES or
    // an SD file. There is no run whose output belongs in this form.
    primary_inputs: &[],
    top_choice: false,
    spec: SpecData {
        summary: "Molecule properties, standardization and substructure search, and explicit bond \
        edits from atom-mapped reactions.",
        description: "RDKit is the open-source cheminformatics toolkit. Given SMILES or an SD \
        file, a run reports canonical structures with descriptors, Lipinski counts, standardized \
        parent forms, InChI, molecule hashes, Murcko scaffolds, ring and stereochemistry analysis, \
        SMARTS substructure matches, fingerprints and the maximum common substructure of the set. \
        Given atom-mapped reaction SMILES, it reports the reaction's participants, its element and \
        formal-charge balance, the quality of its atom mapping, and the bonds formed, broken or \
        changed in order -- the reaction centre a precedent search or an enzyme design starts from.",
        availability: "An ordinary Python dependency of the application, installed with it; the \
        analysis runs in a process of its own. Runs on CPU.",
        license_details: "BSD 3-Clause from the RDKit project. Academic and commercial use are \
        both unrestricted.",
        repo_url: Some("https://github.com/rdkit/rdkit"),
        home_url: Some("https://www.rdkit.org/"),
        docs_url: Some("https://www.rdkit.org/docs/GettingStartedInPython.html"),
        input_params_url: Some("https://www.rdkit.org/docs/source/rdkit.Chem.rdmolfiles.html"),
        examples_url: Some("https://www.rdkit.org/docs/Cookbook.html"),
        paper_url: None,
        license: License::Bsd3Clause,
        license_url: Some("https://github.com/rdkit/rdkit/blob/master/license.txt"),
        tested: true,
    },
};
