use super::*;

struct JobDirectory(std::path::PathBuf);

impl JobDirectory {
    fn new() -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Self(
            std::env::temp_dir().join(format!("bio-tools-presets-{}-{unique}", std::process::id())),
        )
    }
}

impl Drop for JobDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn verify_files(original: &Value, prepared: &Value) -> usize {
    match original {
        Value::String(reference) if reference.starts_with("bio-tools://") => {
            let (slug, name) = reference
                .strip_prefix("bio-tools://")
                .unwrap()
                .split_once('/')
                .unwrap();
            let path = Path::new(prepared.as_str().unwrap());
            assert!(path.is_absolute());
            assert_eq!(
                fs::read_to_string(path).unwrap(),
                asset(slug, name).unwrap()
            );
            1
        }
        Value::String(document) if document.contains("bio-tools://") => verify_files(
            &serde_json::from_str(document).unwrap(),
            &serde_json::from_str(prepared.as_str().unwrap()).unwrap(),
        ),
        Value::Object(entries) => entries
            .iter()
            .map(|(key, value)| verify_files(value, &prepared[key]))
            .sum(),
        Value::Array(entries) => entries
            .iter()
            .zip(prepared.as_array().unwrap())
            .map(|(a, b)| verify_files(a, b))
            .sum(),
        _ => {
            assert_eq!(original, prepared);
            0
        }
    }
}

#[test]
fn every_preset_materializes_all_bundled_inputs() {
    for slug in ["rfd3", "proteinmpnn", "ligandmpnn"] {
        let presets: Value = serde_json::from_str(by_slug(slug).unwrap()).unwrap();
        let mut files = 0;
        for preset in presets.as_array().unwrap() {
            let portable = payload(slug, preset["id"].as_str().unwrap(), &Map::new()).unwrap();
            let directory = JobDirectory::new();
            let prepared = materialize(slug, &portable, &directory.0).unwrap();
            files += verify_files(&portable, &prepared);
            assert_eq!(
                portable,
                payload(slug, preset["id"].as_str().unwrap(), &Map::new()).unwrap()
            );
        }
        assert!(
            files > 20,
            "{slug}: expected to check every structure and auxiliary file"
        );
    }
}

#[test]
fn explicit_overrides_and_ordinary_text_are_preserved() {
    let overrides = serde_json::json!({"input": "", "n_batches": 2, "is_non_loopy": false});
    let values = payload(
        "rfd3",
        "demo/M0255_1mg5_unfixed",
        overrides.as_object().unwrap(),
    )
    .unwrap();
    assert_eq!(values["input"], "");
    assert_eq!(values["n_batches"], 2);
    assert_eq!(values["is_non_loopy"], false);
    assert_eq!(values["length"], "180-200");
    assert_eq!(values["diffusion_batch_size"], 1);
    assert_eq!(
        input_text("rfd3", "ATOM  example\nEND\n").unwrap(),
        "ATOM  example\nEND\n"
    );
    assert!(payload("rfd3", "does-not-exist", &Map::new()).is_err());
}

#[test]
fn only_allowlisted_assets_are_resolved() {
    let directory = JobDirectory::new();
    for reference in [
        "bio-tools://rfd3/../../Cargo.toml",
        "bio-tools://proteinmpnn/LICENSE",
        "bio-tools://rfd3/missing.pdb",
    ] {
        assert!(input_text("rfd3", reference).is_err());
        assert!(materialize("rfd3", &Value::from(reference), &directory.0).is_err());
    }
    assert!(!directory.0.exists());
}

#[test]
fn repeated_references_share_a_file_and_existing_files_survive() {
    let directory = JobDirectory::new();
    fs::create_dir_all(&directory.0).unwrap();
    let existing = directory.0.join("preset-0-M0255_1mg5.pdb");
    fs::write(&existing, "user input").unwrap();
    let reference = "bio-tools://rfd3/input_pdbs/M0255_1mg5.pdb";
    let values = serde_json::json!({"input": reference, "nested": [{"input": reference}]});
    let prepared = materialize("rfd3", &values, &directory.0).unwrap();
    assert_eq!(prepared["input"], prepared["nested"][0]["input"]);
    assert_eq!(verify_files(&values, &prepared), 2);
    assert_eq!(fs::read_to_string(existing).unwrap(), "user input");
}

#[test]
fn protenix_presets_name_known_fields_and_bundled_msas() {
    fn references(value: &Value, found: &mut Vec<String>) {
        match value {
            Value::String(text) if text.starts_with("bio-tools://") => found.push(text.clone()),
            Value::Object(entries) => entries.values().for_each(|entry| references(entry, found)),
            Value::Array(entries) => entries.iter().for_each(|entry| references(entry, found)),
            _ => {}
        }
    }

    let contract: Value =
        serde_json::from_str(super::super::fields::by_slug("protenix").unwrap()).unwrap();
    let mut known: Vec<&str> = contract["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| field["name"].as_str().unwrap())
        .collect();
    known.push(contract["input_modes"]["name"].as_str().unwrap());

    let presets: Value = serde_json::from_str(by_slug("protenix").unwrap()).unwrap();
    let mut bundled = Vec::new();
    for preset in presets.as_array().unwrap() {
        for key in preset["values"].as_object().unwrap().keys() {
            assert!(known.contains(&key.as_str()), "unknown field {key:?}");
        }
        let jobs: Value =
            serde_json::from_str(preset["values"]["input_json"].as_str().unwrap()).unwrap();
        assert!(!jobs.as_array().unwrap().is_empty());
        references(&jobs, &mut bundled);
    }
    assert_eq!(bundled.len(), 7);
    for reference in bundled {
        assert!(input_text("protenix", &reference).unwrap().starts_with('>'));
    }
}

#[test]
fn catpred_presets_name_known_fields_and_valid_reactions() {
    let contract: Value =
        serde_json::from_str(super::super::fields::by_slug("catpred").unwrap()).unwrap();
    let mut known: Vec<&str> = contract["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| field["name"].as_str().unwrap())
        .collect();
    known.push(contract["input_modes"]["name"].as_str().unwrap());

    let presets: Value = serde_json::from_str(by_slug("catpred").unwrap()).unwrap();
    for preset in presets.as_array().unwrap() {
        let values = preset["values"].as_object().unwrap();
        for key in values.keys() {
            assert!(known.contains(&key.as_str()), "unknown field {key:?}");
        }
        // Each preset fills the field its own input mode reads, and every
        // reaction names an enzyme and at least one substrate or inhibitor.
        match values["input_mode"].as_str().unwrap() {
            "parameters" => {
                let boxes: Value =
                    serde_json::from_str(values["sequence_molecules"].as_str().unwrap()).unwrap();
                let kinds: Vec<&str> = boxes
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|entry| entry["type"].as_str().unwrap())
                    .collect();
                assert_eq!(kinds.iter().filter(|kind| **kind == "protein").count(), 1);
                assert!(kinds.contains(&"ligand"));
            }
            "text" => {
                let mut lines = values["input_csv"].as_str().unwrap().lines();
                let header: Vec<&str> = lines.next().unwrap().split(',').collect();
                for column in ["SMILES", "sequence", "pdbpath"] {
                    assert!(header.contains(&column), "missing {column} column");
                }
                assert!(lines.all(|row| row.split(',').count() == header.len()));
            }
            mode => panic!("a preset cannot fill the {mode} mode"),
        }
    }
}
