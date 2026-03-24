// Licensed under the Apache-2.0 license
// SPDX-License-Identifier: Apache-2.0

//! Target communication layer.

mod mctp_transport;
mod traits;

pub use mctp_transport::MctpTransport;
pub use traits::{TargetTransport, TransportError};

/// Echo transport for development and testing.
/// Returns the input as the response without contacting hardware.
#[derive(Default)]
pub struct EchoTransport;

impl TargetTransport for EchoTransport {
    fn send_and_receive(&self, cmd: &str) -> Result<String, TransportError> {
        Ok(format!("echo: {cmd}"))
    }
}
