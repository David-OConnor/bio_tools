use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::{
        Tool,
        catalog::{CatalogEntry, Identity},
    },
};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::EsmFold2),
    categories: &[ToolCategory::StructurePrediction],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Expensive,
    top_choice: false,
    spec: SpecData {
        summary: "Fast all-atom structure prediction for biomolecular complexes.",
        description: "ESMFold2 predicts all-atom structures for protein complexes, DNA, RNA, \
        ligands, modified residues, and covalent complexes through the released 6B-parameter model.",
        availability: "Installed by setup_system.sh into a Python 3.12 uv environment; Biohub/ESMFold2 weights download from Hugging Face on first execution",
        license_details: "The ESM code and released ESMFold2 model are provided under the MIT license, allowing academic and commercial use.",
        repo_url: Some("https://github.com/Biohub/esm"),
        home_url: Some("https://biohub.ai/models/esmfold2"),
        docs_url: Some("https://github.com/Biohub/esm#running-esmfold2-through-hugging-face"),
        input_params_url: Some(
            "https://github.com/Biohub/esm/blob/main/esm/utils/structure/input_builder.py",
        ),
        examples_url: Some("https://github.com/Biohub/esm/tree/main/cookbook/tutorials"),
        paper_url: Some("https://www.biorxiv.org/content/10.64898/2026.06.03.729735v1"),
        license: License::Mit,
        license_url: None,
        tested: true,
    },
};
