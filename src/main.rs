use std::{
    alloc::System,
    env::{self, current_dir},
    ops::Deref,
    path::PathBuf,
    str::FromStr,
};

use anyhow::Result;
use clap::{Arg, Command};

fn main() -> Result<()> {
    let current_dir = "";
    env::set_current_dir(current_dir)?;

    let cmd: Command = Command::new("cozy")
        .bin_name("cozy")
        .subcommand_required(true)
        .subcommand(init(current_dir));

    let matches = cmd.get_matches();

    let (_, matches) = matches.subcommand().unwrap();
    println!(
        "Path := {:?}",
        matches.get_one::<std::path::PathBuf>("path")
    );

    Ok(())
}

fn init(current_dir: &'static str) -> impl Into<Command> {
    Command::new("init").arg(
        Arg::new("path")
            .default_value(current_dir)
            .value_parser(clap::value_parser!(std::path::PathBuf)),
    )
}