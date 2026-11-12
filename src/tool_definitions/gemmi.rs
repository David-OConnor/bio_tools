use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::{
        Tool,
        catalog::{CatalogEntry, Identity},
    },
};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::Gemmi),
    categories: &[ToolCategory::Crystallography],
    launch_type: LaunchType::Executable,
    license_type: LicenseCategory::Copyleft,
    expense: ProcessExpense::Moderate,
    primary_output: None,
    primary_inputs: &[],
    top_choice: false,
    spec: SpecData {
        summary: "Macromolecular crystallography library and utilities",
        description: "Gemmi is a library and command-line program from CCP4 and Global Phasing. \
        It reads and writes coordinates, density maps, MTZ, and structure-factor mmCIF files, and \
        converts between them, e.g. structure factors to maps with `sf2map`.",
        availability: "The `gemmi-program` wheel from PyPI, installed by bio_tools into its own \
        environment on Linux, macOS, and Windows. Runs on CPU.",
        license_details: "MPL 2.0. Commercial use permitted.",
        repo_url: Some("https://github.com/project-gemmi/gemmi"),
        home_url: Some("https://project-gemmi.github.io/"),
        docs_url: Some("https://gemmi.readthedocs.io/en/latest/"),
        input_params_url: Some("https://gemmi.readthedocs.io/en/latest/program.html"),
        examples_url: None,
        paper_url: Some("https://doi.org/10.21105/joss.04200"),
        license: License::Mpl2,
        license_url: None,
        tested: false,
    },
};
