use super::{InstallError, Installer};

pub(super) fn install(installer: &mut Installer) -> Result<(), InstallError> {
    // AlphaFold 3's code and weights have separate licence gates. Creating the interpreter is the
    // only unattended part that both applications can safely share.
    installer.create_venv("alphafold3", "3.11")?;
    installer.note(format!(
        "Created {}. Install a licensed AlphaFold 3 checkout into this interpreter and provide \
         the model parameters separately.",
        installer.venv_dir("alphafold3").display()
    ));
    Ok(())
}
