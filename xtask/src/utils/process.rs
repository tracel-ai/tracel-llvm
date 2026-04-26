use std::process::{Command, Stdio};

use tracel_xtask::prelude::{anyhow::Context as _, *};

pub fn require_tools(tools: &[&str]) -> anyhow::Result<()> {
    for t in tools {
        if which::which(t).is_err() {
            return Err(anyhow::anyhow!(
                "Required tool '{t}' not found in PATH. Install it and retry."
            ));
        }
    }
    Ok(())
}

pub fn run_checked(
    bin: &str,
    args: &[String],
    cwd: Option<&std::path::Path>,
) -> anyhow::Result<()> {
    let mut cmd = Command::new(bin);
    cmd.args(args);
    let joined_args = args.join(" ");
    group_info!("Command line: {} {}", bin, &joined_args);
    if let Some(d) = cwd {
        cmd.current_dir(d);
    }
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());

    let status = cmd
        .status()
        .with_context(|| format!("{bin} should spawn"))?;
    if !status.success() {
        return Err(anyhow::anyhow!(
            "{bin} should succeed (exit status: {status})"
        ));
    }
    Ok(())
}
