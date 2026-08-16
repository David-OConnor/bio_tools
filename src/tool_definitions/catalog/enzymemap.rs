use super::{CatalogEntry, Identity};
use crate::{LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory};

/// EnzymeMap is a downloadable package and bulk dataset, not a hosted query API.
pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Uninstalled {
        slug: "enzymemap",
        name: "EnzymeMap",
    },
    categories: &[
        ToolCategory::Cheminformatics,
        ToolCategory::SequenceAnalysis,
    ],
    launch_type: LaunchType::CondaBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Expensive,
    top_choice: false,
    spec: SpecData {
        summary: "Query or build balanced, atom-mapped enzymatic reaction data.",
        description: "EnzymeMap standardizes BRENDA reactions, resolves structures, balances reactions, atom-maps them, and includes EC and protein information. Its processed reaction table is directly useful as precedent data for reaction search, bond-change extraction, and training synthesis models; the package can also correct and atom-map new enzymatic reactions.",
        availability: "Install from the upstream repository in its supplied Conda environment, or download the processed_reactions.csv.gz dataset from the repository or Zenodo; no unattended bio_tools recipe is provided",
        license_details: "MIT. The downloadable code and processed data repository permit commercial use with attribution; cite the EnzymeMap publication when using the dataset.",
        repo_url: Some("https://github.com/hesther/enzymemap"),
        home_url: Some("https://zenodo.org/doi/10.5281/zenodo.7841848"),
        docs_url: Some("https://github.com/hesther/enzymemap#readme"),
        paper_url: Some("https://doi.org/10.1038/s41467-022-33339-0"),
        license: License::Mit,
        license_url: None,
    },
};
