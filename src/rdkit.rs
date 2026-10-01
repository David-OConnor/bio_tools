//! Local RDKit depiction through an explicitly selected Python interpreter.
//! No network access or installation occurs here; callers own interpreter discovery.

use std::{io, path::Path, time::Duration};

use crate::run::{self, CaptureLimits, CommandSpec};

/// Used by the installer and tool-status probes to verify the drawing backend too.
pub const PROBE: &str = "import rdkit; from rdkit.Chem.Draw import rdMolDraw2D; \
    rdMolDraw2D.MolDraw2DSVG(300, 180); print('RDKit', rdkit.__version__)";

/// Render a SMILES string to SVG with outlined labels, suitable for font-free SVG viewers.
/// Runs on CPU in a bounded subprocess; call from a worker, not the UI thread.
pub fn depict_smiles(python: &Path, smiles: &str, width: u32, height: u32) -> io::Result<Vec<u8>> {
    if smiles.trim().is_empty() || width == 0 || height == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "SMILES and nonzero dimensions are required",
        ));
    }

    let input = serde_json::to_vec(&serde_json::json!({
        "smiles": smiles,
        "width": width,
        "height": height,
    }))?;
    // Ignore inherited PYTHONHOME/PYTHONPATH, but keep the user site-packages directory:
    // system installations made with `pip install --user rdkit` must work too.
    let command = CommandSpec::new(python.as_os_str())
        .args(["-E", "-c", include_str!("rdkit_depict.py")])
        .stdin(input)
        .timeout(Duration::from_secs(30))
        .capture_limits(CaptureLimits::new(2 * 1024 * 1024, 8 * 1024));
    let output = run::run(&command).map_err(|error| io::Error::other(error.to_string()))?;

    // A bounded capture retains the tail; reject a truncated drawing rather than returning it.
    if !output.stdout.windows(4).any(|part| part == b"<svg") {
        return Err(io::Error::other("RDKit returned no complete SVG document"));
    }
    Ok(output.stdout)
}
