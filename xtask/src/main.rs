mod commands;
mod utils;

#[macro_use]
extern crate log;

use std::time::Instant;
use tracel_xtask::prelude::*;

#[macros::base_commands(Publish)]
enum Command {
    /// Generate Rust bindings.
    Bindings(commands::bindings::BindingsCmdArgs),
    /// Generat LLVM bundle.
    Bundle(commands::bundle::BundleCmdArgs),
    #[doc = r"Run checks like formatting, linting etc... This command only reports issues, use the 'fix' command to auto-fix issues."]
    Check(base_commands::check::CheckCmdArgs),
    #[doc = r"Fix issues found with the 'check' command."]
    Fix(base_commands::fix::FixCmdArgs),
    /// Install build prerequisites (cmake, ninja, git, etc.)
    Setup(commands::setup::SetupCmdArgs),
}

fn main() -> anyhow::Result<()> {
    let start = Instant::now();
    let (args, environment) = init_xtask::<Command>(parse_args::<Command>()?)?;
    match args.command {
        Command::Bindings(cmd_args) => commands::bindings::handle_command(cmd_args, environment),
        Command::Bundle(cmd_args) => commands::bundle::handle_command(cmd_args),
        Command::Check(mut cmd_args) => {
            // override the target to avoid using the workspace
            // because we don't want the xtask feature to be defined
            cmd_args.target = Target::AllPackages;
            base_commands::check::handle_command(cmd_args, environment, args.context)
        }
        Command::Fix(mut cmd_args) => {
            // override the target to avoid using the workspace
            // because we don't want the xtask feature to be defined
            cmd_args.target = Target::AllPackages;
            base_commands::fix::handle_command(cmd_args, environment, args.context, None)
        }
        Command::Setup(cmd_args) => commands::setup::handle_command(cmd_args),
        _ => dispatch_base_commands(args, environment),
    }?;
    let duration = start.elapsed();
    info!(
        "\x1B[32;1mTime elapsed for the current execution: {}\x1B[0m",
        format_duration(&duration)
    );
    Ok(())
}
