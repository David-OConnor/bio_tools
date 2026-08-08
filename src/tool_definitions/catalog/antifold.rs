use super::{CatalogEntry, Identity};
use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::Tool,
};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::AntiFold),
    categories: &[
        ToolCategory::AntibodyDesign,
        ToolCategory::SequencePrediction,
    ],
    launch_type: LaunchType::CondaBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Moderate,
    top_choice: false,
    spec: SpecData {
        summary: "Structure-based antibody design using inverse folding",
        description: "AntiFold predicts sequences which fit into an input antibody variable domain structure.
         The tool outputs residue log-likelihoods in CSV format, and can sample sequences to a FASTA format directly.
         Sampled sequences show high structural agreement with experimental structures. \nAntiFold is based on the ESM-IF1 model and is fine-tuned on solved and predicted antibody structures from SAbDab and OAS.",
        availability: "Separate AntiFold, PyTorch, and model installation required",
        license_details: "BSD 3-Clause (Oxford Protein Informatics Group).",
        repo_url: Some("https://github.com/oxpig/AntiFold"),
        home_url: Some("https://opig.stats.ox.ac.uk/webapps/antifold/"),
        docs_url: None,
        paper_url: Some(
            "https://academic.oup.com/bioinformaticsadvances/article/5/1/vbae202/8090019",
        ),
        license: License::Bsd3Clause,
        license_url: None,
    },
};
