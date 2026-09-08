//! Reusable presets and bundled text assets, independent of any web application.
// Source/reference: https://github.com/RosettaCommons/foundry/tree/production/models/rfd3/docs/examples
// Input API: https://rosettacommons.github.io/foundry/models/rfd3/input.html

use std::{collections::BTreeMap, fs, io, path::Path};

use serde_json::{Map, Value};

#[cfg(test)]
mod tests;

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

/// Load a preset with the field catalog's defaults, then apply explicit overrides.
/// Empty strings, false and null are intentional overrides, including for files.
/// File references remain portable until passed to [`materialize`].
pub fn payload(slug: &str, preset_id: &str, overrides: &Map<String, Value>) -> io::Result<Value> {
    let presets: Value = serde_json::from_str(by_slug(slug).unwrap_or("[]"))?;
    let preset = presets
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["id"] == preset_id)
        .ok_or_else(|| invalid(format!("unknown preset {preset_id:?} for {slug:?}")))?;
    let contract: Value = serde_json::from_str(
        super::fields::by_slug(slug)
            .ok_or_else(|| invalid(format!("no field catalog for {slug:?}")))?,
    )?;
    let mut result = Map::new();
    if let Some(modes) = contract.get("input_modes") {
        result.insert(
            modes["name"].as_str().unwrap().into(),
            modes["default"].clone(),
        );
    }
    for field in contract["fields"].as_array().unwrap() {
        result.insert(
            field["name"].as_str().unwrap().into(),
            field["default"].clone(),
        );
    }
    result.extend(preset["values"].as_object().unwrap().clone());
    result.extend(overrides.clone());
    Ok(Value::Object(result))
}

/// Resolve a bundled reference to its contents, or return ordinary input text unchanged.
/// Unknown references and assets belonging to another tool are errors, never local paths.
pub fn input_text<'a>(slug: &str, value: &'a str) -> io::Result<&'a str> {
    let Some(reference) = value.strip_prefix("bio-tools://") else {
        return Ok(value);
    };
    let (owner, name) = reference
        .split_once('/')
        .ok_or_else(|| invalid("invalid bundled asset reference"))?;
    if owner != slug {
        return Err(invalid(format!("expected a bundled {slug} asset")));
    }
    asset(slug, name).ok_or_else(|| invalid(format!("unknown bundled asset {name:?} for {slug:?}")))
}

/// Copy bundled assets into a caller-owned job directory and replace their references
/// with absolute paths. Handles objects, arrays and JSON-encoded form fields (RFD3's
/// multi-design `inputs` and ProteinMPNN's auxiliary files). Ordinary values are retained.
/// The returned payload is a copy; the portable catalog and caller's input are unchanged.
/// No network access or tool installation is needed. Existing files are never overwritten.
pub fn materialize(slug: &str, values: &Value, directory: &Path) -> io::Result<Value> {
    fn visit(
        slug: &str,
        value: &mut Value,
        directory: &Path,
        assets: &mut BTreeMap<String, String>,
    ) -> io::Result<()> {
        match value {
            Value::String(text) if text.starts_with("bio-tools://") => {
                let content = input_text(slug, text)?;
                if let Some(path) = assets.get(text) {
                    *value = Value::String(path.clone());
                    return Ok(());
                }
                fs::create_dir_all(directory)?;
                let directory = directory.canonicalize()?;
                let filename = text.rsplit('/').next().unwrap();
                let mut index = 0;
                let path = loop {
                    let path = directory.join(format!("preset-{index}-{filename}"));
                    match fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&path)
                    {
                        Ok(mut file) => {
                            use io::Write;
                            file.write_all(content.as_bytes())?;
                            break path;
                        }
                        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => index += 1,
                        Err(error) => return Err(error),
                    }
                };
                let path = path
                    .to_str()
                    .ok_or_else(|| invalid("asset path is not UTF-8"))?
                    .to_owned();
                assets.insert(text.clone(), path.clone());
                *value = Value::String(path);
            }
            Value::String(text) if text.contains("bio-tools://") => {
                if let Ok(mut document) = serde_json::from_str::<Value>(text) {
                    if document.is_object() || document.is_array() {
                        visit(slug, &mut document, directory, assets)?;
                        *text = serde_json::to_string_pretty(&document)?;
                    }
                }
            }
            Value::Object(entries) => {
                for entry in entries.values_mut() {
                    visit(slug, entry, directory, assets)?;
                }
            }
            Value::Array(entries) => {
                for entry in entries {
                    visit(slug, entry, directory, assets)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    let mut result = values.clone();
    visit(slug, &mut result, directory, &mut BTreeMap::new())?;
    Ok(result)
}

/// Serialized preset descriptors: id, label, description, source_url and form values.
pub fn by_slug(slug: &str) -> Option<&'static str> {
    match slug {
        "ligandmpnn" => Some(include_str!("ligandmpnn.json")),
        "proteinmpnn" => Some(include_str!("proteinmpnn.json")),
        "rfd3" => Some(include_str!("rfd3.json")),
        _ => None,
    }
}

/// Resolve only explicitly bundled assets; never interpret a caller's name as a path.
/// Consumers can write these contents into their own job directory before inference.
pub fn asset(slug: &str, name: &str) -> Option<&'static str> {
    match (slug, name) {
        ("ligandmpnn", "inputs/1BC8.pdb") => Some(include_str!("ligandmpnn/inputs/1BC8.pdb")),
        ("ligandmpnn", "inputs/2GFB.pdb") => Some(include_str!("ligandmpnn/inputs/2GFB.pdb")),
        ("ligandmpnn", "inputs/4GYT.pdb") => Some(include_str!("ligandmpnn/inputs/4GYT.pdb")),
        ("ligandmpnn", "inputs/bias_AA_per_residue.json") => {
            Some(include_str!("ligandmpnn/inputs/bias_AA_per_residue.json"))
        }
        ("ligandmpnn", "inputs/bias_AA_per_residue_multi.json") => Some(include_str!(
            "ligandmpnn/inputs/bias_AA_per_residue_multi.json"
        )),
        ("ligandmpnn", "inputs/fix_residues_multi.json") => {
            Some(include_str!("ligandmpnn/inputs/fix_residues_multi.json"))
        }
        ("ligandmpnn", "inputs/omit_AA_per_residue.json") => {
            Some(include_str!("ligandmpnn/inputs/omit_AA_per_residue.json"))
        }
        ("ligandmpnn", "inputs/omit_AA_per_residue_multi.json") => Some(include_str!(
            "ligandmpnn/inputs/omit_AA_per_residue_multi.json"
        )),
        ("ligandmpnn", "inputs/pdb_ids.json") => {
            Some(include_str!("ligandmpnn/inputs/pdb_ids.json"))
        }
        ("ligandmpnn", "inputs/redesigned_residues_multi.json") => Some(include_str!(
            "ligandmpnn/inputs/redesigned_residues_multi.json"
        )),
        ("ligandmpnn", "LICENSE") => Some(include_str!("ligandmpnn/LICENSE")),
        ("ligandmpnn", "outputs/ligandmpnn_default/backbones/1BC8_1.pdb") => Some(include_str!(
            "ligandmpnn/outputs/ligandmpnn_default/backbones/1BC8_1.pdb"
        )),
        ("ligandmpnn", "provenance.json") => Some(include_str!("ligandmpnn/provenance.json")),

        ("proteinmpnn", "inputs/PDB_complexes/pdbs/3HTN.pdb") => Some(include_str!(
            "proteinmpnn/inputs/PDB_complexes/pdbs/3HTN.pdb"
        )),
        ("proteinmpnn", "inputs/PDB_complexes/pdbs/4YOW.pdb") => Some(include_str!(
            "proteinmpnn/inputs/PDB_complexes/pdbs/4YOW.pdb"
        )),
        ("proteinmpnn", "inputs/PDB_homooligomers/pdbs/4GYT.pdb") => Some(include_str!(
            "proteinmpnn/inputs/PDB_homooligomers/pdbs/4GYT.pdb"
        )),
        ("proteinmpnn", "inputs/PDB_homooligomers/pdbs/6EHB.pdb") => Some(include_str!(
            "proteinmpnn/inputs/PDB_homooligomers/pdbs/6EHB.pdb"
        )),
        ("proteinmpnn", "inputs/PDB_monomers/pdbs/5L33.pdb") => Some(include_str!(
            "proteinmpnn/inputs/PDB_monomers/pdbs/5L33.pdb"
        )),
        ("proteinmpnn", "inputs/PDB_monomers/pdbs/6MRR.pdb") => Some(include_str!(
            "proteinmpnn/inputs/PDB_monomers/pdbs/6MRR.pdb"
        )),
        ("proteinmpnn", "LICENSE") => Some(include_str!("proteinmpnn/LICENSE")),
        ("proteinmpnn", "outputs/example_3_outputs/seqs/3HTN.fa") => Some(include_str!(
            "proteinmpnn/outputs/example_3_outputs/seqs/3HTN.fa"
        )),
        ("proteinmpnn", "inputs/PSSM_inputs/3HTN.json") => {
            Some(include_str!("proteinmpnn/inputs/PSSM_inputs/3HTN.json"))
        }
        ("proteinmpnn", "inputs/PSSM_inputs/4YOW.json") => {
            Some(include_str!("proteinmpnn/inputs/PSSM_inputs/4YOW.json"))
        }
        ("proteinmpnn", "provenance.json") => Some(include_str!("proteinmpnn/provenance.json")),
        ("rfd3", "LICENSE.md") => Some(include_str!("rfd3/LICENSE.md")),
        ("rfd3", "input_pdbs/1bna.pdb") => Some(include_str!("rfd3/input_pdbs/1bna.pdb")),
        ("rfd3", "input_pdbs/1q75.pdb") => Some(include_str!("rfd3/input_pdbs/1q75.pdb")),
        ("rfd3", "input_pdbs/2r5z.pdb") => Some(include_str!("rfd3/input_pdbs/2r5z.pdb")),
        ("rfd3", "input_pdbs/4zxb_cropped.pdb") => {
            Some(include_str!("rfd3/input_pdbs/4zxb_cropped.pdb"))
        }
        ("rfd3", "input_pdbs/5o45_cropped.pdb") => {
            Some(include_str!("rfd3/input_pdbs/5o45_cropped.pdb"))
        }
        ("rfd3", "input_pdbs/5o4d.pdb") => Some(include_str!("rfd3/input_pdbs/5o4d.pdb")),
        ("rfd3", "input_pdbs/7v11.pdb") => Some(include_str!("rfd3/input_pdbs/7v11.pdb")),
        ("rfd3", "input_pdbs/IAI.pdb") => Some(include_str!("rfd3/input_pdbs/IAI.pdb")),
        ("rfd3", "input_pdbs/M0255_1mg5.pdb") => {
            Some(include_str!("rfd3/input_pdbs/M0255_1mg5.pdb"))
        }
        ("rfd3", "input_pdbs/symmetry_examples/1bfr_C2.pdb") => Some(include_str!(
            "rfd3/input_pdbs/symmetry_examples/1bfr_C2.pdb"
        )),
        ("rfd3", "input_pdbs/symmetry_examples/1e3v_C2.pdb") => Some(include_str!(
            "rfd3/input_pdbs/symmetry_examples/1e3v_C2.pdb"
        )),
        ("rfd3", "input_pdbs/symmetry_examples/1j79_C2.pdb") => Some(include_str!(
            "rfd3/input_pdbs/symmetry_examples/1j79_C2.pdb"
        )),
        ("rfd3", "input_pdbs/symmetry_examples/6t8h_C3.pdb") => Some(include_str!(
            "rfd3/input_pdbs/symmetry_examples/6t8h_C3.pdb"
        )),
        ("rfd3", "provenance.json") => Some(include_str!("rfd3/provenance.json")),
        _ => None,
    }
}
