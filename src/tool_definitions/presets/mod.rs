//! Reusable presets and bundled text assets, independent of any web application.
// Source/reference: https://github.com/RosettaCommons/foundry/tree/production/models/rfd3/docs/examples
// Input API: https://rosettacommons.github.io/foundry/models/rfd3/input.html

/// Serialized preset descriptors: id, label, description, source_url and form values.
pub fn by_slug(slug: &str) -> Option<&'static str> {
    match slug {
        "rfd3" => Some(include_str!("rfd3.json")),
        _ => None,
    }
}

/// Resolve only explicitly bundled assets; never interpret a caller's name as a path.
/// Consumers can write these contents into their own job directory before inference.
pub fn asset(slug: &str, name: &str) -> Option<&'static str> {
    match (slug, name) {
        ("rfd3", "LICENSE.md") => Some(include_str!("rfd3/LICENSE.md")),
        ("rfd3", "input_pdbs/1bna.pdb") => Some(include_str!("rfd3/input_pdbs/1bna.pdb")),
        ("rfd3", "input_pdbs/1q75.pdb") => Some(include_str!("rfd3/input_pdbs/1q75.pdb")),
        ("rfd3", "input_pdbs/2r5z.pdb") => Some(include_str!("rfd3/input_pdbs/2r5z.pdb")),
        ("rfd3", "input_pdbs/4zxb_cropped.pdb") => Some(include_str!("rfd3/input_pdbs/4zxb_cropped.pdb")),
        ("rfd3", "input_pdbs/5o45_cropped.pdb") => Some(include_str!("rfd3/input_pdbs/5o45_cropped.pdb")),
        ("rfd3", "input_pdbs/5o4d.pdb") => Some(include_str!("rfd3/input_pdbs/5o4d.pdb")),
        ("rfd3", "input_pdbs/7v11.pdb") => Some(include_str!("rfd3/input_pdbs/7v11.pdb")),
        ("rfd3", "input_pdbs/IAI.pdb") => Some(include_str!("rfd3/input_pdbs/IAI.pdb")),
        ("rfd3", "input_pdbs/M0255_1mg5.pdb") => Some(include_str!("rfd3/input_pdbs/M0255_1mg5.pdb")),
        ("rfd3", "input_pdbs/symmetry_examples/1bfr_C2.pdb") => Some(include_str!("rfd3/input_pdbs/symmetry_examples/1bfr_C2.pdb")),
        ("rfd3", "input_pdbs/symmetry_examples/1e3v_C2.pdb") => Some(include_str!("rfd3/input_pdbs/symmetry_examples/1e3v_C2.pdb")),
        ("rfd3", "input_pdbs/symmetry_examples/1j79_C2.pdb") => Some(include_str!("rfd3/input_pdbs/symmetry_examples/1j79_C2.pdb")),
        ("rfd3", "input_pdbs/symmetry_examples/6t8h_C3.pdb") => Some(include_str!("rfd3/input_pdbs/symmetry_examples/6t8h_C3.pdb")),
        ("rfd3", "provenance.json") => Some(include_str!("rfd3/provenance.json")),
        _ => None,
    }
}
