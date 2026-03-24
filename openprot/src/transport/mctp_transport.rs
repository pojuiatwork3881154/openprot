// Licensed under the Apache-2.0 license
// SPDX-License-Identifier: Apache-2.0

//! MCTP transport using mctp-lib with a loopback mock for testing.
//!
//! Uses two routers (client + echo server) with BufferSenders that capture
//! packets. Packets are fed between routers in-process to simulate a roundtrip.

use std::sync::{Arc, Mutex};

use mctp::{Eid, MsgIC, MsgType, Tag};
use mctp_lib::fragment::SendOutput;
use mctp_lib::{Router, Sender};

use super::traits::{TargetTransport, TransportError};

const MTU: usize = 255;
const MAX_LISTENERS: usize = 8;
const MAX_REQUESTS: usize = 8;

/// Buffer sender that captures MCTP packets to a shared buffer.
struct BufferSender {
    packets: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl BufferSender {
    fn new(packets: Arc<Mutex<Vec<Vec<u8>>>>) -> Self {
        Self { packets }
    }
}

impl Sender for BufferSender {
    fn send_vectored(
        &mut self,
        _eid: Eid,
        mut fragmenter: mctp_lib::fragment::Fragmenter,
        payload: &[&[u8]],
    ) -> mctp::Result<mctp::Tag> {
        loop {
            let mut buf = [0u8; MTU];
            match fragmenter.fragment_vectored(payload, &mut buf) {
                SendOutput::Packet(items) => {
                    self.packets
                        .lock()
                        .map_err(|_| mctp::Error::InternalError)?
                        .push(items.into());
                }
                SendOutput::Complete { tag, cookie: _ } => return Ok(tag),
                SendOutput::Error { err, cookie: _ } => return Err(err),
            }
        }
    }

    fn get_mtu(&self) -> usize {
        MTU
    }
}

/// MCTP transport with in-process loopback for testing.
///
/// Client router sends to server router; server echoes the payload back.
/// Use with `--target mctp` when running the openprot binary.
pub struct MctpTransport {
    client_router: Mutex<Router<BufferSender, MAX_LISTENERS, MAX_REQUESTS>>,
    server_router: Mutex<Router<BufferSender, MAX_LISTENERS, MAX_REQUESTS>>,
    client_packets: Arc<Mutex<Vec<Vec<u8>>>>,
    server_packets: Arc<Mutex<Vec<Vec<u8>>>>,
    server_eid: Eid,
    msg_type: MsgType,
}

impl MctpTransport {
    /// Create a new MCTP transport with loopback echo server.
    ///
    /// - `client_eid`: Our endpoint ID (e.g. 8)
    /// - `server_eid`: Echo server endpoint ID (e.g. 42)
    /// - `msg_type`: MCTP message type for commands (e.g. MsgType(1))
    pub fn new(client_eid: Eid, server_eid: Eid, msg_type: MsgType) -> mctp::Result<Self> {
        let client_packets = Arc::new(Mutex::new(Vec::new()));
        let server_packets = Arc::new(Mutex::new(Vec::new()));

        let client_sender = BufferSender::new(Arc::clone(&client_packets));
        let server_sender = BufferSender::new(Arc::clone(&server_packets));

        let client_router = Router::new(client_eid, 0, client_sender);
        let mut server_router = Router::new(server_eid, 0, server_sender);

        server_router.listener(msg_type)?;

        Ok(Self {
            client_router: Mutex::new(client_router),
            server_router: Mutex::new(server_router),
            client_packets,
            server_packets,
            server_eid,
            msg_type,
        })
    }

    /// Feed client packets to server and run echo logic.
    fn run_server_echo(
        &self,
        client_packets: Vec<Vec<u8>>,
    ) -> Result<Vec<Vec<u8>>, TransportError> {
        let mut server_router = self
            .server_router
            .lock()
            .map_err(|e| TransportError(e.to_string()))?;

        for pkt in &client_packets {
            if let Some(cookie) = server_router
                .inbound(pkt)
                .map_err(|e| TransportError(format!("{e:?}")))?
            {
                let (source, typ, tag, ic, payload) = match server_router.recv(cookie) {
                    Some(msg) => {
                        let payload: Vec<u8> = msg.payload.into();
                        let tag = Tag::Unowned(msg.tag.tag());
                        (msg.source, msg.typ, tag, msg.ic, payload)
                    }
                    None => continue,
                };
                server_router
                    .send(Some(source), typ, Some(tag), ic, cookie, &payload)
                    .map_err(|e| TransportError(format!("{e:?}")))?;
            }
        }

        drop(server_router);
        Ok(self
            .server_packets
            .lock()
            .map_err(|e| TransportError(e.to_string()))?
            .drain(..)
            .collect())
    }

    /// Feed server response packets to client.
    fn feed_to_client(
        &self,
        server_packets: Vec<Vec<u8>>,
    ) -> Result<(), TransportError> {
        let mut client_router = self
            .client_router
            .lock()
            .map_err(|e| TransportError(e.to_string()))?;

        for pkt in &server_packets {
            client_router
                .inbound(pkt)
                .map_err(|e| TransportError(format!("{e:?}")))?;
        }

        Ok(())
    }
}

impl TargetTransport for MctpTransport {
    fn send_and_receive(&self, cmd: &str) -> Result<String, TransportError> {
        let cookie = {
            let mut client_router = self
                .client_router
                .lock()
                .map_err(|e| TransportError(e.to_string()))?;

            let cookie = client_router
                .req(self.server_eid)
                .map_err(|e| TransportError(format!("{e:?}")))?;

            client_router
                .send(
                    None,
                    self.msg_type,
                    None,
                    MsgIC(false),
                    cookie,
                    cmd.as_bytes(),
                )
                .map_err(|e| TransportError(format!("{e:?}")))?;

            cookie
        };

        let client_packets: Vec<Vec<u8>> = self
            .client_packets
            .lock()
            .map_err(|e| TransportError(e.to_string()))?
            .drain(..)
            .collect();

        let server_packets = self.run_server_echo(client_packets)?;
        self.feed_to_client(server_packets)?;

        let mut client_router = self
            .client_router
            .lock()
            .map_err(|e| TransportError(e.to_string()))?;

        let payload: Vec<u8> = {
            let msg = client_router
                .recv(cookie)
                .ok_or_else(|| TransportError("no response received".to_string()))?;
            msg.payload.into()
        };
        client_router
            .unbind(cookie)
            .map_err(|e| TransportError(format!("{e:?}")))?;

        String::from_utf8(payload).map_err(|e| TransportError(e.to_string()))
    }
}
