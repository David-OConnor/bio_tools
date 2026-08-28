use super::{CatalogEntry, Identity};
use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::Tool,
};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::Rfd3),
    categories: &[
        ToolCategory::ProteinDesign,
        ToolCategory::PeptideBinderDesign,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Expensive,
    top_choice: true,
    spec: SpecData {
        summary: "Generates all-atom protein backbones around proteins, small molecules, nucleic acids, and metals.",
        description: "RFdiffusion3 is the successor to RFdiffusion, rebuilt on the AtomWorks/RF3 stack \
        rather than on RoseTTAFold2's SE(3) trunk. It diffuses every atom instead of a residue frame, so \
        one model covers cases the original needed separate checkpoints or no support at all for: enzyme \
        active sites with their cofactors, small-molecule and nucleic-acid binders, and metal sites. \
        Constraints are given as a JSON or YAML specification rather than as command-line contigs, and a \
        single unified selection language decides, per atom, what is held fixed -- coordinates, sequence, \
        both, or neither. Fixing sequence but not structure turns it into a predictor, fixing backbone but \
        not sequence into an inverse-folding model, and unfixing only side chains into a packer.",
        availability: "Installed by setup_system.sh, which fetches the public model checkpoint; an NVIDIA GPU is required",
        license_details: "BSD 3-Clause from the Institute for Protein Design, University of Washington, covering the inference code, the training code, and the public checkpoint alike: unrestricted academic and commercial use.",
        repo_url: Some("https://github.com/RosettaCommons/foundry"),
        home_url: Some("https://rosettacommons.github.io/foundry/models/rfd3/index.html"),
        docs_url: Some("https://rosettacommons.github.io/foundry/models/rfd3/input.html"),
        paper_url: Some("https://doi.org/10.1101/2025.09.18.676967"),
        license: License::Bsd3Clause,
        license_url: None,
    },
};
