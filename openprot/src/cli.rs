// Licensed under the Apache-2.0 license
// SPDX-License-Identifier: Apache-2.0

//! CLI definition using clap.

use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "openprot")]
#[command(about = "OpenPRoT host tool")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Default: print greeting
    Greet {
        #[arg(default_value = "OpenProt")]
        name: String,
    },

    /// Interactive mode: read from terminal, send to target
    Interact(InteractArgs),
}

#[derive(Args)]
pub struct InteractArgs {
    /// Target transport (e.g. serial port). Use "echo" for mock.
    #[arg(long, default_value = "echo")]
    pub target: String,
}
