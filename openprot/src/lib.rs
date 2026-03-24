// Licensed under the Apache-2.0 license
// SPDX-License-Identifier: Apache-2.0

pub mod cli;
pub mod commands;
pub mod transport;

pub use cli::{Cli, Commands, InteractArgs};
pub use transport::{EchoTransport, MctpTransport, TargetTransport, TransportError};

pub fn greet(name: &str) -> String {
    format!("Hello, {name}!")
}

#[cfg(test)]
mod tests {
    use mctp::{Eid, MsgType};

    use super::*;

    #[test]
    fn test_greet() {
        assert_eq!(greet("OpenProt"), "Hello, OpenProt!");
    }

    #[test]
    fn test_mctp_transport_echo() {
        let transport = MctpTransport::new(Eid(8), Eid(42), MsgType(1)).unwrap();
        let response = transport.send_and_receive("hello").unwrap();
        assert_eq!(response, "hello");
    }

    #[test]
    fn test_mctp_transport_echo_twice() {
        let transport = MctpTransport::new(Eid(8), Eid(42), MsgType(1)).unwrap();
        assert_eq!(transport.send_and_receive("hello").unwrap(), "hello");
        assert_eq!(transport.send_and_receive("world").unwrap(), "world");
    }
}
