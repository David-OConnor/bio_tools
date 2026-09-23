use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::{
        Tool,
        catalog::{CatalogEntry, DataType, Identity, PrimaryInput},
    },
};

/// Runs on Linux and on Windows alike. `rc-foundry` is a pure-Python wheel, and resolving
/// `rc-foundry[rfd3]` for the two platforms gives the same packages: the only dependencies it
/// gates on Linux are NVIDIA's fused cuEquivariance kernels, and those belong to the `rf3` and
/// `all` extras, so neither platform installs them here. RFD3 imports them behind a `try` and
/// falls back to its own PyTorch attention regardless. Nothing in RFD3 calls `torch.compile`
/// either, so Triton being Linux-only costs nothing.
///
/// One dependency does need help: atomworks applies its conformer-generation time limit in a
/// `fork`ed child, and Windows has no such start method, so the recipe patches that time limit.
/// Without it a protein-only design still succeeds, while anything carrying a ligand or a
/// nucleic acid dies in `CreateDesignReferenceFeatures`. See
/// `install::python_tools::patch_atomworks_timeout` and the script it runs,
/// `install/patches/atomworks_fork_free_timeout.py`, for the whole argument.
///
/// Reference:
///   foundry, which publishes RFD3   https://github.com/RosettaCommons/foundry
///   the `rc-foundry` distribution   https://pypi.org/project/rc-foundry/
///   installing it                   https://rosettacommons.github.io/foundry/models/rfd3/tutorials/RFdiffusion3_installation_tutorial.html
///   install troubleshooting         https://rosettacommons.github.io/foundry/installation_faq.html
///   atomworks, the data layer       https://github.com/RosettaCommons/atomworks
///   NVIDIA cuEquivariance           https://github.com/NVIDIA/cuEquivariance
pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::RfDiffusion3),
    categories: &[
        ToolCategory::ProteinDesign,
        ToolCategory::BackboneGeneration,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Expensive,
    primary_output: Some(DataType::MmCif),
    primary_inputs: &[PrimaryInput::new(
        "input",
        &[DataType::Pdb, DataType::MmCif],
    )],
    top_choice: true,
    spec: SpecData {
        summary: "Generates protein backbone coordinates around proteins, small molecules, \
         nucleic acids, and metals. Given geometric and other constraints, specifies backbone geometry. \
          A useful first step in a protein design pipeline. Very well documented!",
        description: "RFdiffusion3 is a practical first step in protein design workflows: It generates coordinates \
        of the backbone atoms for a protein based on contraints (For example, to spacially deconflict with specific \
        molecules). Its output can be fed into ProteinMPNN or LigandMPNN to generate an amino acid sequence.",
        availability: "Installed by setup_system.sh, which fetches the public model checkpoint; an NVIDIA GPU is required. \
         Runs on Windows as well as Linux, from the same set of packages.",
        license_details: "BSD 3-Clause from the Institute for Protein Design, University of Washington, covering the inference code, the training code, and the public checkpoint alike: unrestricted academic and commercial use.",
        repo_url: Some("https://github.com/RosettaCommons/foundry"),
        home_url: Some(
            "https://github.com/RosettaCommons/foundry/blob/production/models/rfd3/README.md",
        ),
        docs_url: Some("https://rosettacommons.github.io/foundry/models/rfd3/index.html"),
        input_params_url: Some(
            "https://rosettacommons.github.io/foundry/models/rfd3/input.html#inputspecification-fields",
        ),
        examples_url: Some(
            "https://github.com/RosettaCommons/foundry/tree/production/models/rfd3/docs/examples",
        ),
        paper_url: Some("https://doi.org/10.1101/2025.09.18.676967"),
        license: License::Bsd3Clause,
        license_url: None,
        tested: true,
    },
};
