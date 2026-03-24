// Copyright 2025
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use embedded_io_adapters::std::FromStd;
use mctp::Error;
use std::io::Write;

use mctp_lib::{Sender, fragment::SendOutput, serial::MctpSerialHandler};

pub struct IoSerialSender<W: Write> {
    writer: FromStd<W>,
    serial_handler: MctpSerialHandler,
}
impl<W: Write> IoSerialSender<W> {
    pub fn new(writer: W) -> Self {
        IoSerialSender {
            writer: FromStd::new(writer),
            serial_handler: MctpSerialHandler::new(),
        }
    }
}

impl<W: Write> Sender for IoSerialSender<W> {
    fn send_vectored(
        &mut self,
        _eid: mctp::Eid,
        mut fragmenter: mctp_lib::fragment::Fragmenter,
        payload: &[&[u8]],
    ) -> mctp::Result<mctp::Tag> {
        loop {
            let mut pkt = [0; mctp_lib::serial::MTU_MAX];
            let fragment = fragmenter.fragment_vectored(payload, &mut pkt);
            match fragment {
                SendOutput::Packet(items) => {
                    self.serial_handler.send_sync(items, &mut self.writer)?;
                    self.writer.inner_mut().flush().map_err(Error::Io)?;
                }
                SendOutput::Complete { tag, cookie: _ } => return Ok(tag),
                SendOutput::Error { err, cookie: _ } => return Err(err),
            }
        }
    }

    fn get_mtu(&self) -> usize {
        mctp_lib::serial::MTU_MAX
    }
}
