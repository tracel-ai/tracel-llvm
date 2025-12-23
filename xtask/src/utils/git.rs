use std::path::Path;

use crate::utils::process::run_checked;
use tracel_xtask::prelude::*;

pub fn git_clone_shallow_tag(repo: &str, tag: &str, dest: &Path) -> anyhow::Result<()> {
    run_checked(
        "git",
        &[
            "clone".into(),
            "--depth=1".into(),
            "--branch".into(),
            tag.into(),
            repo.into(),
            dest.to_string_lossy().into_owned(),
        ],
        None,
    )
}
