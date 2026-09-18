use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::{
        Tool,
        catalog::{CatalogEntry, DataType, Identity, PrimaryInput},
    },
};

/// A BLAST search against a few hundred germline segments: the process start
/// and the database load dominate, and a repertoire of thousands of reads
/// still finishes in the time one fold spends allocating.
pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::IgBlast),
    categories: &[ToolCategory::SequenceAnalysis],
    launch_type: LaunchType::Executable,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Moderate,
    // The AIRR Rearrangement TSV `-outfmt 19` writes: one row per query with
    // the V/D/J/C calls, the junction, and every FWR/CDR region, which is what
    // the repertoire tooling downstream of IgBLAST reads.
    primary_output: Some(DataType::Tsv),
    // The query FASTA, taken whole: IgBLAST annotates every record in it, so a
    // designed or predicted sequence arrives with its header intact.
    primary_inputs: &[PrimaryInput::new(
        "input_fasta",
        &[DataType::AaSequence, DataType::DnaSequence],
    )],
    top_choice: false,
    spec: SpecData {
        summary: "Annotate antibody and T cell receptor sequences against germline V, D and J genes.",
        description: "IgBLAST is NCBI's aligner for immunoglobulin and T cell receptor variable \
        domains. It reports the top germline V, D, J and constant-region matches for each query, \
        the rearrangement summary (chain type, stop codons, V-J frame, productivity), the V-D and \
        D-J junction details with their N/P nucleotides, and the framework and complementarity \
        determining regions under either the IMGT or the Kabat delineation. Nucleotide queries run \
        through igblastn and protein queries through igblastp; a nucleotide run also writes the \
        AIRR Rearrangement TSV and, on request, a clonotype summary of the whole repertoire.",
        availability: "The NCBI IgBLAST distribution and its prebuilt AIRR-C human, AIRR-C mouse, \
        NCBI mouse, rhesus monkey and human constant-region germline databases are installed by \
        setup_system.sh. Runs on CPU; no GPU is used.",
        license_details: "Public domain: a US Government work from NCBI, with no copyright \
        asserted and no restriction on commercial use. The IMGT-derived and AIRR-C/OGRDB germline \
        sets carry their own attribution terms.",
        repo_url: Some("https://github.com/ncbi/igblast"),
        home_url: Some("https://www.ncbi.nlm.nih.gov/igblast/"),
        docs_url: Some("https://ncbi.github.io/igblast/"),
        input_params_url: Some("https://ncbi.github.io/igblast/cook/How-to-set-up.html"),
        examples_url: Some("https://ncbi.github.io/igblast/cook/examples.html"),
        paper_url: Some("https://doi.org/10.1093/nar/gkt382"),
        license: License::PublicDomain,
        license_url: Some("https://ncbi.github.io/igblast/dev/copyright.html"),
        tested: true,
    },
};
