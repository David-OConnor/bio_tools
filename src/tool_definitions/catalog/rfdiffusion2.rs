use super::{CatalogEntry, Identity};
use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::Tool,
};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::RfDiffusion2),
    categories: &[
        ToolCategory::ProteinDesign,
        ToolCategory::PeptideBinderDesign,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Expensive,
    top_choice: false,
    spec: SpecData {
        summary: "Scaffolds enzyme active sites from individual atoms, without being told where in \
        the chain they belong.",
        description: "RFdiffusion2 keeps RFdiffusion's RoseTTAFold-based diffusion but changes what a \
        motif is. The original held whole residues at positions the contig fixed in advance; RFdiffusion2 \
        holds arbitrary sets of atoms -- the tip atoms of a catalytic triad, say -- and, with guideposts \
        enabled, decides for itself which residues of the new chain carry them and in what order. That \
        removes the combinatorial search over residue indices and orderings that made enzyme scaffolding \
        with the original impractical, and it is what the tool is for: active sites specified as atoms \
        plus their cofactors and substrates. Small-molecule binder design is supported too, conditioned on \
        a target relative solvent-accessible surface area, along with ORI tokens that pin the scaffold's \
        centre of mass. Upstream recommends the original RFdiffusion, not this, for protein binder design.",
        availability: "Installed by setup_system.sh, which clones the repository and fetches the public \
        model weights; Linux/WSL and an NVIDIA GPU are required",
        license_details: "BSD 3-Clause from the Institute for Protein Design, University of Washington, \
        covering the inference code and the public checkpoints alike: unrestricted academic and commercial \
        use. PyRosetta, which the upstream environment lists and which is separately licensed, is imported \
        lazily and is not needed for inference.",
        repo_url: Some("https://github.com/RosettaCommons/RFdiffusion2"),
        home_url: Some("https://rosettacommons.github.io/RFdiffusion2/"),
        docs_url: Some("https://rosettacommons.github.io/RFdiffusion2/readme_link.html"),
        paper_url: Some("https://doi.org/10.1101/2025.04.09.648075"),
        license: License::Bsd3Clause,
        license_url: Some("https://github.com/RosettaCommons/RFdiffusion2/blob/main/LICENSE.md"),
    },
};
