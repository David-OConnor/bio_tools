//! Shared scientific adapters used by desktop and web clients.
//!
//! Python owns input validation and native command construction; execution and
//! durable artifacts use the bio_tools Python binding's CommandSpec.
use std::{fs, io, path::{Path, PathBuf}, hash::{Hash, Hasher}};

const FILES: &[(&str, &[u8])] = &[
    ("bio_tool_adapters/desktop.py", include_bytes!("python/bio_tool_adapters/desktop.py")),
    ("bio_tool_adapters/__init__.py", include_bytes!("python/bio_tool_adapters/__init__.py")),
    ("bio_tool_adapters/boltz2.py", include_bytes!("python/bio_tool_adapters/boltz2.py")),
    ("bio_tool_adapters/chai1.py", include_bytes!("python/bio_tool_adapters/chai1.py")),
    ("bio_tool_adapters/environments.py", include_bytes!("python/bio_tool_adapters/environments.py")),
    ("bio_tool_adapters/esmc.py", include_bytes!("python/bio_tool_adapters/esmc.py")),
    ("bio_tool_adapters/tool_scripts/esmc_inference.py", include_bytes!("python/bio_tool_adapters/tool_scripts/esmc_inference.py")),
    ("bio_tool_adapters/esmfold2.py", include_bytes!("python/bio_tool_adapters/esmfold2.py")),
    ("bio_tool_adapters/field_processing.py", include_bytes!("python/bio_tool_adapters/field_processing.py")),
    ("bio_tool_adapters/ligandmpnn.py", include_bytes!("python/bio_tool_adapters/ligandmpnn.py")),
    ("bio_tool_adapters/opendde.py", include_bytes!("python/bio_tool_adapters/opendde.py")),
    ("bio_tool_adapters/proteinmpnn.py", include_bytes!("python/bio_tool_adapters/proteinmpnn.py")),
    ("bio_tool_adapters/protenix.py", include_bytes!("python/bio_tool_adapters/protenix.py")),
    ("bio_tool_adapters/rfdiffusion3.py", include_bytes!("python/bio_tool_adapters/rfdiffusion3.py")),
    ("bio_tool_adapters/status_check.py", include_bytes!("python/bio_tool_adapters/status_check.py")),
    ("bio_tool_adapters/tool_scripts/esmfold2_inference.py", include_bytes!("python/bio_tool_adapters/tool_scripts/esmfold2_inference.py")),
];

/// Materialize the versioned adapter package. Large example data, such as Chai-1's MSAs, is not
/// embedded; the installer downloads it instead.
pub fn package_path() -> io::Result<PathBuf> {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    for (name, data) in FILES { name.hash(&mut hash); data.hash(&mut hash); }
    let root = std::env::temp_dir().join(format!("bio-tools-adapters-{:x}", hash.finish()));
    write_package(&root)?;
    Ok(root)
}

pub fn write_package(root: &Path) -> io::Result<()> {
    for (name, data) in FILES {
        let path = root.join(name);
        if fs::read(&path).ok().as_deref() == Some(*data) { continue; }
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(path, data)?;
    }
    Ok(())
}
