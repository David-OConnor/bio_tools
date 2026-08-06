use std::process::Command;

use super::{InstallError, Installer, common::PipOptions};
use crate::tool_definitions::Tool;

pub(super) fn install(installer: &mut Installer) -> Result<(), InstallError> {
    const SLUG: &str = Tool::BoltzGen.slug();
    installer.create_venv(SLUG, "3.12")?;
    installer.pip_install(SLUG, &["boltzgen~=0.3.2"], PipOptions::default())?;
    let mut verify = Command::new(installer.venv_script(SLUG, Tool::BoltzGen.console_script()));
    verify.arg("--help");
    installer.checked(&mut verify)
}
