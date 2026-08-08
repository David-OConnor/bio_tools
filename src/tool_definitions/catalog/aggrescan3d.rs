use super::{CatalogEntry, Identity};
use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::Tool,
};

/// A single static structure is seconds; the CABS-flex ensemble and the
/// mutation search the form also offer are not.
pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::AggreScan3d),
    categories: &[ToolCategory::PropertyPrediction],
    launch_type: LaunchType::CondaBasedApp,
    license_type: LicenseCategory::NonCommercial,
    expense: ProcessExpense::Expensive,
    top_choice: false,
    spec: SpecData {
        summary: "Map aggregation-prone patches onto a protein structure and score them.",
        description: "Runs the AggreScan3D standalone package, which projects intrinsic aggregation propensities onto a structure and weights them by solvent exposure, giving a per-residue score and a total for the protein. The dynamic mode reruns the analysis over a CABS-flex ensemble instead of the single deposited conformation.",
        availability: "Installed by setup_system.sh into a Conda environment (the upstream package is Python 2.7-only); the mutate and auto_mutation modes additionally need a licensed FoldX install, and the dynamic mode needs CABS-flex, neither of which setup_system.sh installs",
        license_details: "Free for academic use (Laboratory of Computational Biology, University of Warsaw). The mutate and auto_mutation modes call FoldX, which needs its own academic licence key; the dynamic mode calls CABS-flex, from the same lab. Commercial use of either requires a separate agreement.",
        repo_url: Some("https://bitbucket.org/lcbio/aggrescan3d"),
        home_url: Some("https://biocomp.chem.uw.edu.pl/A3D2/"),
        docs_url: Some("https://academic.oup.com/nar/article/47/W1/W300/5485072"),
        paper_url: Some("https://academic.oup.com/nar/article/47/W1/W300/5485072"),
        license: License::Other,
        license_url: None,
    },
};
