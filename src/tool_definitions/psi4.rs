use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::{
        Tool,
        catalog::{CatalogEntry, Identity},
    },
};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::Psi4),
    categories: &[ToolCategory::QuantumChemistry],
    launch_type: LaunchType::Executable,
    license_type: LicenseCategory::Copyleft,
    expense: ProcessExpense::Expensive,
    primary_output: None,
    primary_inputs: &[],
    top_choice: false,
    spec: SpecData {
        summary: "Open-source ab initio quantum chemistry",
        description: "Run Psithon inputs or PsiAPI scripts: energies, gradients, optimizations, \
            frequencies and properties with Hartree–Fock, DFT, coupled-cluster, SAPT, \
            multireference and other methods.",
        availability: "Managed CPU installation on x64 and ARM64 Linux and macOS, and x64 Windows. \
            Windows 11 on ARM64 runs the x64 build under emulation. Includes the DFT-D3, DFT-D4, \
            gCP, geomeTRIC, EFP and PCMSolver add-ons, plus adcc and ddX on Linux and macOS.",
        license_details: "LGPL 3.0. Bundled dependencies have their own licenses.",
        repo_url: Some("https://github.com/psi4/psi4/"),
        home_url: Some("https://psicode.org/"),
        docs_url: Some("https://psicode.org/psi4manual/master/index.html"),
        input_params_url: Some("https://psicode.org/psi4manual/master/tutorial.html"),
        examples_url: Some("https://github.com/psi4/psi4/tree/v1.11/samples"),
        paper_url: Some("https://doi.org/10.1063/5.0006002"),
        license: License::Lgpl3Only,
        license_url: Some("https://github.com/psi4/psi4/blob/v1.11/COPYING.LESSER"),
        tested: true,
    },
};
