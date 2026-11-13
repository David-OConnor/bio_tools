use std::{fmt::Write, sync::OnceLock};

use bio_files::{
    Sdf,
    gromacs::{MoleculeInput, gro::make_gro},
};
use dynamics::{
    ForceFieldFamily, MolDynamics, WaterModel,
    gromacs::make_topology,
    params::{FfParamSet, ParameterizationOptions},
};
use pyo3::{exceptions::PyValueError, prelude::*};

static AMBER: OnceLock<FfParamSet> = OnceLock::new();
static OPLS: OnceLock<FfParamSet> = OnceLock::new();

#[pyfunction]
pub fn gromacs_prepare_ligands(
    py: Python<'_>,
    records: Vec<(String, i32)>,
    force_field: &str,
    water_model: &str,
    solvate: bool,
) -> PyResult<(String, String, &'static str)> {
    py.detach(|| prepare(&records, force_field, water_model, solvate))
        .map_err(PyValueError::new_err)
}

fn prepare(
    records: &[(String, i32)],
    force_field: &str,
    water_model: &str,
    solvate: bool,
) -> Result<(String, String, &'static str), String> {
    if records.is_empty() {
        return Err("Supply at least one 3D molecule.".into());
    }

    let (family, cache) = match force_field {
        "amber99sb-ildn" | "amber03" | "amber99sb" => (ForceFieldFamily::Amber, &AMBER),
        "oplsaa" => (ForceFieldFamily::Opls, &OPLS),
        _ => return Err("Choose Amber (GAFF2) or OPLS-AA to prepare small molecules.".into()),
    };
    let set = if let Some(set) = cache.get() {
        set
    } else {
        let set = FfParamSet::from_family(family).map_err(|e| e.to_string())?;
        cache.get_or_init(|| set)
    };

    let mut resolved = Vec::with_capacity(records.len());
    let mut coordinates = Vec::with_capacity(records.len());
    let mut names = Vec::with_capacity(records.len());
    let mut next_x = 0.0;
    for (i, (record, formal_charge)) in records.iter().enumerate() {
        let sdf = Sdf::new(record).map_err(|e| e.to_string())?;
        let molecule = MolDynamics::from_sdf(&sdf, None);
        let mut molecule = set
            .parameterize_with_options(&molecule, &ParameterizationOptions::local_only())
            .map_err(|e| {
                format!(
                    "Could not parameterize molecule {} with {}: {}",
                    i + 1,
                    if family == ForceFieldFamily::Amber {
                        "GAFF2"
                    } else {
                        "OPLS-AA"
                    },
                    e
                )
            })?;
        let total_charge: f64 = molecule
            .atoms
            .iter()
            .filter_map(|atom| atom.partial_charge.map(f64::from))
            .sum();
        // Project inferred partial charges onto the molecule's formal net charge.
        let charge_offset =
            (f64::from(*formal_charge) - total_charge) / molecule.atoms.len() as f64;
        let name = format!("M{}", i + 1);
        let mut atom_names = Vec::with_capacity(molecule.atoms.len());
        let shift = if records.len() > 1 {
            let min = molecule
                .atoms
                .iter()
                .map(|a| a.posit.x)
                .fold(f64::INFINITY, f64::min);
            next_x - min
        } else {
            0.0
        };
        for (j, atom) in molecule.atoms.iter_mut().enumerate() {
            // Bio Files uses the force-field name as the GRO atom name for ligands.
            let mut atom_name = format!("{}{}", atom.element.to_letter(), j + 1);
            atom_name.truncate(5);
            atom.partial_charge = atom
                .partial_charge
                .map(|charge| (f64::from(charge) + charge_offset) as f32);
            atom_names.push(atom_name.clone());
            atom.posit.x += shift;
            next_x = next_x.max(atom.posit.x);
        }
        next_x += 3.0;

        let mut atoms = molecule.atoms.clone();
        for (atom, atom_name) in atoms.iter_mut().zip(&atom_names) {
            atom.hetero = true;
            atom.force_field_type = Some(atom_name.clone());
        }
        coordinates.push(MoleculeInput {
            name: name.clone(),
            atoms,
            bonds: Vec::new(),
            ff_params: None,
            count: 1,
            copy_atom_posits: None,
        });
        resolved.push((name, molecule, 1));
        names.push(atom_names);
    }

    let water = ligand_water(set, water_model)?;
    let template = if water.o_m_dist == 0.0 {
        "spc216.gro"
    } else {
        "tip4p.gro"
    };
    let topology =
        make_topology(&resolved, set, Some(&water), solvate).map_err(|e| e.to_string())?;
    let gro = make_gro(&coordinates, &None).map_err(|e| e.to_string())?;

    Ok((name_topology_atoms(&topology, &names), gro, template))
}

fn ligand_water(set: &FfParamSet, model: &str) -> Result<WaterModel, String> {
    let mut water = set.default_water.clone();
    let (oh, angle, charge, sigma, epsilon) = match model {
        "auto" | "none" => return Ok(water),
        "tip3p" => (0.9572, 104.52_f32, 0.417, 3.15061, 0.636386),
        "spce" => (1.0, 109.47_f32, 0.4238, 3.16557, 0.650194),
        _ => return Err("Unknown water model.".into()),
    };
    water.o_h_dist = oh;
    water.o_m_dist = 0.0;
    water.h_o_h_angle = angle.to_radians();
    water.q_h = charge;
    water.lj_sigma_o = sigma;
    water.lj_eps_o = epsilon / 4.184;
    water.lj_sigma_h = 0.0;
    water.lj_eps_h = 0.0;
    if set.family == ForceFieldFamily::Amber {
        // Joung–Cheatham monovalent ions from Amber's model-specific frcmod files.
        let (na, cl) = if model == "tip3p" {
            ((1.369, 0.0874393), (2.513, 0.0355910))
        } else {
            ((1.212, 0.3526418), (2.711, 0.0127850))
        };
        let sigma_factor = 2.0 / 2.0_f32.powf(1.0 / 6.0);
        (water.cation.lj_sigma, water.cation.lj_eps) = (na.0 * sigma_factor, na.1);
        (water.anion.lj_sigma, water.anion.lj_eps) = (cl.0 * sigma_factor, cl.1);
    }

    Ok(water)
}

// Give the topology the same element-prefixed atom names as the GRO.
fn name_topology_atoms(topology: &str, names: &[Vec<String>]) -> String {
    let mut result = String::with_capacity(topology.len());
    let mut in_atoms = false;
    for line in topology.lines() {
        if line.starts_with('[') {
            in_atoms = line == "[ atoms ]";
        }
        if in_atoms {
            let fields: Vec<_> = line.split_whitespace().collect();
            if let [index, kind, resid, resname, _, group, charge, mass] = fields.as_slice()
                && let Some((molecule, atom)) = kind
                    .strip_prefix("DYN_")
                    .and_then(|kind| kind.split_once('_'))
                && let (Ok(molecule), Ok(atom)) = (molecule.parse::<usize>(), atom.parse::<usize>())
            {
                let name = &names[molecule][atom];
                writeln!(
                    result,
                    "{index} {kind} {resid} {resname} {name} {group} {charge} {mass}"
                )
                .unwrap();
                continue;
            }
        }
        writeln!(result, "{line}").unwrap();
    }
    result
}
