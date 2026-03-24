// Licensed under the Apache-2.0 license
// SPDX-License-Identifier: Apache-2.0

//! Interact subcommand: read from terminal, send to target.

use std::io::{self, BufRead, Write};

use mctp::{Eid, MsgType};

use crate::cli::InteractArgs;
use crate::transport::{EchoTransport, MctpTransport, TargetTransport};

/// Run the interactive loop.
pub fn run(args: &InteractArgs) -> std::io::Result<()> {
    let transport: Box<dyn TargetTransport> = match args.target.as_str() {
        "echo" => Box::new(EchoTransport),
        "mctp" => {
            let mctp = MctpTransport::new(Eid(8), Eid(42), MsgType(1))
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e:?}")))?;
            Box::new(mctp)
        }
        _ => {
            eprintln!("Unknown target '{}', using echo transport", args.target);
            Box::new(EchoTransport)
        }
    };

    let mut stdin = io::stdin().lock();
    let mut stdout = io::stdout().lock();

    writeln!(stdout, "OpenPRoT interactive mode. Type commands, Ctrl+D to exit.")?;
    loop {
        write!(stdout, "> ")?;
        stdout.flush()?;

        let mut line = String::new();
        if stdin.read_line(&mut line)? == 0 {
            break;
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        match transport.send_and_receive(line) {
            // Prefix so loopback MCTP (reply == command) is visible after the "> " prompt.
            Ok(response) => writeln!(stdout, "< {response}")?,
            Err(e) => writeln!(stdout, "Error: {e}")?,
        }
        stdout.flush()?;
    }
    Ok(())
}
