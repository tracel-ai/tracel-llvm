use std::path::{Path, PathBuf};

use clap::Args;
use tracel_xtask::prelude::*;

use crate::utils::process::run_checked;

#[derive(Args)]
pub struct SetupCmdArgs {
    /// Path to the scripts directory (default: ./scripts)
    #[arg(long, default_value = "scripts")]
    scripts_dir: PathBuf,
}

pub fn handle_command(args: SetupCmdArgs) -> anyhow::Result<()> {
    let scripts_dir = args.scripts_dir;

    if cfg!(windows) {
        run_windows(&scripts_dir)
    } else {
        run_unix(&scripts_dir)
    }
}

fn run_unix(scripts_dir: &Path) -> anyhow::Result<()> {
    let script = scripts_dir.join("install-prereqs.sh");

    if !script.exists() {
        return Err(anyhow::anyhow!(
            "Prerequisite script not found: {}",
            script.display()
        ));
    }

    group_info!("xtask setup: installing prerequisites (unix)");
    run_checked("bash", &[script.to_string_lossy().into_owned()], None)?;
    endgroup!();

    Ok(())
}

fn run_windows(scripts_dir: &Path) -> anyhow::Result<()> {
    let script = scripts_dir.join("install-prereqs.ps1");

    if !script.exists() {
        return Err(anyhow::anyhow!(
            "Prerequisite script not found: {}",
            script.display()
        ));
    }

    group_info!("xtask setup: installing prerequisites (windows)");
    run_checked(
        "powershell",
        &[
            "-NoProfile".into(),
            "-ExecutionPolicy".into(),
            "Bypass".into(),
            "-File".into(),
            script.to_string_lossy().into_owned(),
        ],
        None,
    )?;
    endgroup!();

    Ok(())
}
