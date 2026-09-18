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

#[test]
fn igblast_presets_name_known_fields_and_valid_queries() {
    let contract: Value =
        serde_json::from_str(super::super::fields::by_slug("igblast").unwrap()).unwrap();
    let mut known: Vec<&str> = contract["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| field["name"].as_str().unwrap())
        .collect();
    known.push(contract["input_modes"]["name"].as_str().unwrap());

    let presets: Value = serde_json::from_str(by_slug("igblast").unwrap()).unwrap();
    let entries = presets.as_array().unwrap();
    assert!(entries.len() >= 4);
    for preset in entries {
        let values = preset["values"].as_object().unwrap();
        for key in values.keys() {
            assert!(known.contains(&key.as_str()), "unknown field {key:?}");
        }
        assert!(
            preset["source_url"]
                .as_str()
                .unwrap()
                .starts_with("https://")
        );

        // Every preset fills the field its own input mode reads, with FASTA
        // records whose letters match the program it selects.
        assert_eq!(values["input_mode"].as_str().unwrap(), "text");
        let query = values["input_fasta"].as_str().unwrap();
        assert!(
            query.starts_with('>'),
            "a preset query needs a FASTA header"
        );
        let protein = values["sequence_type"].as_str().unwrap() == "protein";
        let residues: String = query
            .lines()
            .filter(|line| !line.starts_with('>'))
            .flat_map(str::chars)
            .collect();
        assert!(!residues.is_empty());
        let nucleotides = residues.chars().all(|base| "ACGTN".contains(base));
        assert_eq!(
            protein, !nucleotides,
            "{}: the query letters and the program disagree",
            preset["id"]
        );
        // A protein query reaches igblastp, which takes no D, J or C database
        // and writes no AIRR table.
        if protein {
            assert_eq!(values["write_airr"], false);
            assert_eq!(values["num_clonotype"], 0);
        }
        // Whatever the preset asks for, a run of it produces something.
        assert!(
            values["write_airr"] == true || !values["report_format"].as_str().unwrap().is_empty()
        );
    }

    // The catalog's own defaults survive, and an override still wins.
    let overrides = serde_json::json!({"organism": "mouse", "num_threads": 1});
    let filled = payload("igblast", "human_igh_airr", overrides.as_object().unwrap()).unwrap();
    assert_eq!(filled["organism"], "mouse");
    assert_eq!(filled["num_threads"], 1);
    assert_eq!(filled["domain_system"], "imgt");
    assert_eq!(filled["min_d_match"], 5);
    assert_eq!(filled["germline_db_v"], "auto");
    assert!(payload("igblast", "does-not-exist", &Map::new()).is_err());
}

#[test]
fn rdkit_presets_name_known_fields_and_cover_both_tasks() {
    let contract: Value =
        serde_json::from_str(super::super::fields::by_slug("rdkit").unwrap()).unwrap();
    let mut known: Vec<&str> = contract["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| field["name"].as_str().unwrap())
        .collect();
    known.push(contract["input_modes"]["name"].as_str().unwrap());
    // A preset says which task it is for, so choosing one switches the form.
    known.push("task");

    let tasks: Vec<&str> = contract["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|task| task["value"].as_str().unwrap())
        .collect();

    let presets: Value = serde_json::from_str(by_slug("rdkit").unwrap()).unwrap();
    let entries = presets.as_array().unwrap();
    assert!(entries.len() >= 4);
    let mut covered: Vec<&str> = Vec::new();
    for preset in entries {
        let values = preset["values"].as_object().unwrap();
        for key in values.keys() {
            assert!(known.contains(&key.as_str()), "unknown field {key:?}");
        }
        assert!(
            preset["source_url"]
                .as_str()
                .unwrap()
                .starts_with("https://www.rdkit.org/docs/")
        );
        let task = values["task"].as_str().unwrap();
        assert!(tasks.contains(&task), "unknown task {task:?}");
        if !covered.contains(&task) {
            covered.push(task);
        }
        assert_eq!(values["input_mode"].as_str().unwrap(), "text");

        // Every preset fills the field its own task reads, with input the task
        // can actually be given.
        if task == "molecule" {
            let molecules: Vec<&str> = values["smiles"]
                .as_str()
                .unwrap()
                .lines()
                .filter(|line| !line.trim().is_empty())
                .collect();
            assert!(!molecules.is_empty());
            assert!(molecules.iter().all(|line| !line.contains(">>")));
        } else {
            let reactions: Vec<&str> = values["reaction_smiles"]
                .as_str()
                .unwrap()
                .lines()
                .filter(|line| !line.trim().is_empty())
                .collect();
            assert!(!reactions.is_empty());
            // Atom maps are what the bond-edit extraction needs, and every
            // reaction preset is meant to demonstrate it.
            assert!(reactions.iter().all(|line| line.contains(">>")));
            assert!(reactions.iter().all(|line| line.contains(":1]")));
        }
    }
    assert_eq!(covered.len(), tasks.len(), "a task has no preset");

    // The catalog's own defaults survive, and an override still wins.
    let overrides = serde_json::json!({"fingerprint": "maccs", "descriptor_set": "all"});
    let filled = payload("rdkit", "descriptors", overrides.as_object().unwrap()).unwrap();
    assert_eq!(filled["fingerprint"], "maccs");
    assert_eq!(filled["descriptor_set"], "all");
    assert_eq!(filled["include_inchi"], true);
    assert_eq!(filled["mcs_timeout"], 20);
    assert_eq!(filled["fingerprint_radius"], 2);
    assert!(payload("rdkit", "does-not-exist", &Map::new()).is_err());
}
