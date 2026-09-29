//! Git engine for Git Ronin.
//!
//! Reads go through gix. Anything that mutates the repository or talks to the
//! network goes through the system `git` binary, so hooks, credential helpers,
//! signing and user config behave exactly as they do on the command line.

mod cli;
mod error;
mod repo;

pub use cli::{GitCli, GitVersion, MIN_GIT_VERSION};
pub use error::{Error, Result};
pub use repo::{HeadState, RepoInfo, open_repo};
