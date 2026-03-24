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

//! Example that listens for a request and echoes the payload in the response.
//!
//! Uses the standalone std implementation for the Stack and attaches to a specified serial port.
//! (Use a tool like _socat_ to attach to the linux MCTP stack through PTYs)
//!
//! Errors after the specified timeout.

const MSG_TYPE: MsgType = MsgType(1);
const OWN_EID: Eid = Eid(8);
const TIMEOUT_SECS: u64 = 10;
const TTY_PATH: &str = "pts1";

use std::{fs::File, thread::spawn, time::Duration};

use mctp::{Eid, Listener, MsgType, RespChannel};
use standalone::{
    Stack,
    serial_sender::IoSerialSender,
    util::{inbound_loop, update_loop},
};

fn main() {
    let serial = File::options()
        .write(true)
        .read(true)
        .open(TTY_PATH)
        .unwrap();

    let serial_sender = IoSerialSender::new(serial.try_clone().unwrap());

    let mut stack = Stack::new(serial_sender);

    stack.set_eid(OWN_EID).unwrap();

    let update_stack = stack.clone();
    spawn(move || update_loop(update_stack));

    let driver_stack = stack.clone();
    spawn(move || inbound_loop(driver_stack, serial));

    let mut listener = stack
        .listener(MSG_TYPE, Some(Duration::from_secs(TIMEOUT_SECS)))
        .unwrap();

    let mut buf = [0; 256];
    let (_, _, msg, mut rsp) = listener.recv(&mut buf).unwrap();

    println!("Got message: {:#x?}", msg);

    rsp.send(msg).unwrap();
}
