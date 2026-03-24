// Licensed under the Apache-2.0 license
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
use openprot::cli::{Cli, Commands};
use openprot::commands::interact;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Greet { name } => {
            println!("{}", openprot::greet(&name));
        }
        Commands::Interact(args) => {
            interact::run(&args)?;
        }
    }
    Ok(())
}
