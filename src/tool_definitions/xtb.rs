use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::{
        Tool,
        catalog::{CatalogEntry, Identity},
    },
};

/// Single points on small systems take milliseconds; large systems and long trajectories take
/// minutes or more.
pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::Xtb),
    categories: &[
        ToolCategory::QuantumChemistry,
        ToolCategory::MoleculeDynamics,
    ],
    launch_type: LaunchType::Executable,
    license_type: LicenseCategory::Copyleft,
    expense: ProcessExpense::Moderate,
    primary_output: None,
    primary_inputs: &[],
    top_choice: false,
    spec: SpecData {
        summary: "Semiempirical Extended Tight-Binding Program Package",
        description: "xtb is the semiempirical program package developed by the Grimme group in \
        Bonn. It provides the GFNn-xTB quantum mechanical methods and the GFN-FF force field.",
        availability: "The official release binaries are installed by bio_tools on Linux, macOS, \
        and Windows. Runs on CPU; no GPU is used.",
        license_details: "LGPL 3.0 or later. Release archives also bundle OpenBLAS or Intel MKL \
        and compiler runtimes, under the licences in share/licenses.",
        repo_url: Some("https://github.com/grimme-lab/xtb"),
        home_url: None,
        docs_url: Some("https://xtb-docs.readthedocs.io/en/latest/"),
        input_params_url: Some("https://xtb-docs.readthedocs.io/en/latest/commandline.html"),
        examples_url: Some("https://xtb-docs.readthedocs.io/en/latest/gfnff.html"),
        paper_url: Some("https://doi.org/10.1002/wcms.1493"),
        license: License::Lgpl3OrLater,
        license_url: None,
        tested: false,
    },
};
