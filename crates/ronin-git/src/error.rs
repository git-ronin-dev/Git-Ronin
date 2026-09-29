use std::path::PathBuf;

use crate::{GitVersion, MIN_GIT_VERSION};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("git executable not found; install git {MIN_GIT_VERSION} or newer")]
    GitNotFound,
    #[error("git {found} is too old; Git Ronin needs {MIN_GIT_VERSION} or newer")]
    GitTooOld { found: GitVersion },
    #[error("`git {args}` failed ({code}): {stderr}")]
    Command {
        args: String,
        code: String,
        stderr: String,
    },
    #[error("not a git repository: {}", .0.display())]
    NotARepo(PathBuf),
    #[error("git: {0}")]
    Gix(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// gix has a distinct error type per operation; the UI only needs the message.
pub(crate) fn gix_err(err: impl std::error::Error) -> Error {
    Error::Gix(err.to_string())
}
