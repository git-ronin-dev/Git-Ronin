//! Tags. Pushing and deleting them on a remote lives in `remote`.

use std::path::Path;

use crate::cli::operand;
use crate::repo::workdir;
use crate::{Error, GitCli, Result};

/// Creates tag `name` at `target`: annotated when `message` has text,
/// lightweight otherwise. Annotated tags are signed if `tag.gpgSign` says so.
pub fn create_tag(
    git: &GitCli,
    path: &Path,
    name: &str,
    target: &str,
    message: Option<&str>,
) -> Result<()> {
    let workdir = workdir(path)?;
    let invalid = || Error::Invalid(format!("“{name}” is not a valid tag name"));
    let name = operand(name).map_err(|_| invalid())?;
    let full = format!("refs/tags/{name}");
    if !git
        .run_raw(&workdir, ["check-ref-format", &full])?
        .success()
    {
        return Err(invalid());
    }
    let target = operand(target)?;
    match message.map(str::trim).filter(|m| !m.is_empty()) {
        Some(message) => git.run_with_input(
            &workdir,
            ["tag", "--annotate", "--file=-", name, target],
            message.as_bytes(),
        )?,
        None => git.run(&workdir, ["tag", name, target])?,
    };
    Ok(())
}

pub fn delete_tag(git: &GitCli, path: &Path, name: &str) -> Result<()> {
    let name = operand(name)?;
    git.run(&workdir(path)?, ["tag", "--delete", name])?;
    Ok(())
}
