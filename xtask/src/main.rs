mod commands;
mod utils;

#[macro_use]
extern crate log;

use std::time::Instant;
use tracel_xtask::prelude::*;

#[macros::base_commands(Build, Bump, Check, Compile, Fix, Test, Publish)]
enum Command {
    /// Generate Rust bindings.
    Bindgen(commands::bindgen::BindgenCmdArgs),
    /// Generat LLVM bundle.
    Bundle(commands::bundle::BundleCmdArgs),
    /// Install build prerequisites (cmake, ninja, git, etc.)
    Setup(commands::setup::SetupCmdArgs),
}

fn main() -> anyhow::Result<()> {
    let start = Instant::now();
    let (args, environment) = init_xtask::<Command>(parse_args::<Command>()?)?;
    match args.command {
        Command::Bindgen(cmd_args) => commands::bindgen::handle_command(cmd_args),
        Command::Bundle(cmd_args) => commands::bundle::handle_command(cmd_args),
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
