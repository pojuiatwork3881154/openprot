// Licensed under the Apache-2.0 license
// SPDX-License-Identifier: Apache-2.0

//! Target communication abstraction.
//!
//! Minimal trait for sending commands to and receiving responses from the target.
//! Implementations: mock (echo), serial, TCP, etc.

use std::fmt;

/// Error from target communication.
#[derive(Debug)]
pub struct TransportError(pub String);

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for TransportError {}

/// Abstraction for communicating with the target device.
pub trait TargetTransport: Send {
    /// Send a command and receive the response.
    fn send_and_receive(&self, cmd: &str) -> Result<String, TransportError>;
}
