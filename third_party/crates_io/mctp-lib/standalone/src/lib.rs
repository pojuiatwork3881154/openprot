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

//! Standalone implementation of mctp-lib usind [std] platform abstactions.
//!
//! Intended for use in examples and tests.

pub mod serial_sender;

use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use mctp::{Eid, Error, Listener, MsgIC, MsgType, ReqChannel, RespChannel, Tag};
use mctp_lib::{AppCookie, Router, Sender};

const MAX_LISTENER_HANDLES: usize = 128;
const MAX_REQ_HANDLES: usize = 128;

/// STD MCTP stack
///
/// Encapsulates a inner [Router] in a thread safe and sharable manner.
/// Provides implementations for the [mctp] traits that hold references to the `stack`.
pub struct Stack<S: Sender> {
    inner: Arc<Mutex<Router<S, MAX_LISTENER_HANDLES, MAX_REQ_HANDLES>>>,
    /// Notifiers to inform _requests_ and _listeners_ about new messages.
    notifiers: Arc<Mutex<HashMap<AppCookie, Arc<Condvar>>>>,
    start_time: Instant,
}

/// A request implementing [ReqChannel]
#[derive(Debug)]
pub struct Request<S: Sender> {
    /// Thread safe reference to a stack
    stack: Arc<Mutex<Router<S, MAX_LISTENER_HANDLES, MAX_REQ_HANDLES>>>,
    cookie: AppCookie,
    /// The [Condvar] that nofifies the request once the response is available
    notifier: Arc<Condvar>,
    timeout: Option<Duration>,
    tag: Option<Tag>,
}
/// A listener implementing [Listener]
#[derive(Debug)]
pub struct ReqListener<S: Sender> {
    stack: Arc<Mutex<Router<S, MAX_LISTENER_HANDLES, MAX_REQ_HANDLES>>>,
    notifiers: Arc<Mutex<HashMap<AppCookie, Arc<Condvar>>>>,
    cookie: AppCookie,
    notifier: Arc<Condvar>,
    timeout: Option<Duration>,
}
/// A response for a request received by a [ReqListener]
#[derive(Debug)]
pub struct Response<S: Sender> {
    stack: Arc<Mutex<Router<S, MAX_LISTENER_HANDLES, MAX_REQ_HANDLES>>>,
    notifiers: Arc<Mutex<HashMap<AppCookie, Arc<Condvar>>>>,
    tag: Tag,
    typ: MsgType,
    remote_eid: Eid,
}

impl<S: Sender> Stack<S> {
    pub fn new(outbound: S) -> Self {
        let inner = Router::new(Eid(0), 0, outbound);
        Self {
            inner: Arc::new(Mutex::new(inner)),
            notifiers: Arc::new(Mutex::new(HashMap::new())),
            start_time: Instant::now(),
        }
    }
    pub fn request(&mut self, dest: Eid, timeout: Option<Duration>) -> mctp::Result<Request<S>> {
        let handle = self
            .inner
            .lock()
            .map_err(|_| Error::InternalError)?
            .req(dest)?;
        let mut notifiers = self.notifiers.lock().map_err(|_| Error::InternalError)?;
        let notifier = Arc::new(Condvar::new());
        notifiers.insert(handle, Arc::clone(&notifier));
        Ok(Request {
            stack: self.inner.clone(),
            cookie: handle,
            notifier,
            timeout,
            tag: None,
        })
    }
    pub fn listener(
        &mut self,
        typ: MsgType,
        timeout: Option<Duration>,
    ) -> mctp::Result<ReqListener<S>> {
        let handle = self
            .inner
            .lock()
            .map_err(|_| Error::InternalError)?
            .listener(typ)?;
        let mut notifiers = self.notifiers.lock().map_err(|_| Error::InternalError)?;
        let notifier = Arc::new(Condvar::new());
        notifiers.insert(handle, Arc::clone(&notifier));
        Ok(ReqListener {
            stack: self.inner.clone(),
            cookie: handle,
            notifier,
            timeout,
            notifiers: Arc::clone(&self.notifiers),
        })
    }

    pub fn inbound(&mut self, pkt: &[u8]) -> Result<(), Error> {
        let cookie = self
            .inner
            .lock()
            .map_err(|_| Error::InternalError)?
            .inbound(pkt)?;
        if let Some(handle) = cookie {
            let notifiers = self.notifiers.lock().map_err(|_| Error::InternalError)?;
            let notifier = notifiers.get(&handle);
            notifier.inspect(|c| c.notify_all());
        }
        Ok(())
    }

    /// Call the update function of the inner stack with the current timestamp
    ///
    /// Convenience function that gets the current timestamp by calculating the duration since the stack was initialized (using [std::time]).
    pub fn update(&mut self) -> Result<u64, Error> {
        self.inner
            .lock()
            .map_err(|_| Error::InternalError)?
            .update(Instant::now().duration_since(self.start_time).as_millis() as u64)
    }

    /// Set the stacks EID
    pub fn set_eid(&mut self, eid: Eid) -> Result<(), Error> {
        self.inner
            .lock()
            .map_err(|_| Error::InternalError)?
            .set_eid(eid)
    }
}

impl<S: Sender> Clone for Stack<S> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            notifiers: Arc::clone(&self.notifiers),
            start_time: self.start_time,
        }
    }
}

impl<S: Sender> ReqChannel for Request<S> {
    fn send_vectored(
        &mut self,
        typ: mctp::MsgType,
        integrity_check: mctp::MsgIC,
        bufs: &[&[u8]],
    ) -> mctp::Result<()> {
        let tag = self
            .stack
            .lock()
            .map_err(|_| Error::InternalError)?
            .send_vectored(None, typ, None, integrity_check, self.cookie, bufs)?;
        self.tag = Some(tag);
        Ok(())
    }

    fn recv<'f>(
        &mut self,
        buf: &'f mut [u8],
    ) -> mctp::Result<(mctp::MsgType, mctp::MsgIC, &'f mut [u8])> {
        let Some(tag) = self.tag else {
            return Err(Error::BadArgument);
        };
        let mut stack = self.stack.lock().unwrap();
        loop {
            if let Some(mut msg) = stack.recv(self.cookie) {
                if msg.tag.tag() != tag.tag() {
                    msg.retain();
                    return Err(Error::InternalError);
                }
                buf.get_mut(..msg.payload.len())
                    .ok_or(Error::NoSpace)?
                    .copy_from_slice(msg.payload);
                return Ok((msg.typ, msg.ic, &mut buf[..msg.payload.len()]));
            }
            if let Some(timeout) = self.timeout {
                let (stack_result, timeout_result) =
                    self.notifier.wait_timeout(stack, timeout).unwrap();
                if timeout_result.timed_out() {
                    return Err(Error::TimedOut);
                } else {
                    stack = stack_result;
                }
            } else {
                stack = self.notifier.wait(stack).unwrap();
            }
        }
    }

    fn remote_eid(&self) -> Eid {
        todo!()
    }
}

impl<S: Sender> Listener for ReqListener<S> {
    type RespChannel<'a>
        = Response<S>
    where
        Self: 'a;

    fn recv<'f>(
        &mut self,
        buf: &'f mut [u8],
    ) -> mctp::Result<(MsgType, MsgIC, &'f mut [u8], Self::RespChannel<'_>)> {
        let mut stack = self.stack.lock().unwrap();
        loop {
            if let Some(msg) = stack.recv(self.cookie) {
                buf.get_mut(..msg.payload.len())
                    .ok_or(Error::NoSpace)?
                    .copy_from_slice(msg.payload);
                let resp = Response {
                    stack: Arc::clone(&self.stack),
                    tag: Tag::Unowned(msg.tag.tag()),
                    remote_eid: msg.source,
                    typ: msg.typ,
                    notifiers: Arc::clone(&self.notifiers),
                };
                return Ok((msg.typ, msg.ic, &mut buf[..msg.payload.len()], resp));
            }
            if let Some(timeout) = self.timeout {
                let (stack_result, timeout_result) =
                    self.notifier.wait_timeout(stack, timeout).unwrap();
                if timeout_result.timed_out() {
                    return Err(Error::TimedOut);
                } else {
                    stack = stack_result;
                }
            } else {
                stack = self.notifier.wait(stack).unwrap();
            }
        }
    }
}

impl<S: Sender> RespChannel for Response<S> {
    type ReqChannel = Request<S>;

    fn send_vectored(&mut self, integrity_check: MsgIC, bufs: &[&[u8]]) -> mctp::Result<()> {
        self.stack
            .lock()
            .map_err(|_| Error::InternalError)?
            .send_vectored(
                Some(self.remote_eid),
                self.typ,
                Some(self.tag),
                integrity_check,
                AppCookie(255), // TODO improve this in mctp-lib
                bufs,
            )?;
        Ok(())
    }

    fn remote_eid(&self) -> Eid {
        self.remote_eid
    }

    fn req_channel(&self) -> mctp::Result<Self::ReqChannel> {
        let handle = self
            .stack
            .lock()
            .map_err(|_| Error::InternalError)?
            .req(self.remote_eid)?;
        let mut notifiers = self.notifiers.lock().map_err(|_| Error::InternalError)?;
        let notifier = Arc::new(Condvar::new());
        notifiers.insert(handle, Arc::clone(&notifier));
        Ok(Request {
            stack: self.stack.clone(),
            cookie: handle,
            notifier,
            timeout: None,
            tag: None,
        })
    }
}

pub mod util {
    use std::{
        io::{BufReader, Read},
        thread::sleep,
        time::Duration,
    };

    use crate::Stack;
    use embedded_io_adapters::std::FromStd;
    use mctp_lib::Sender;

    /// Loop that updates the `stack` periodically
    ///
    /// The stack gets updated atleast once every 100 ms.
    pub fn update_loop<S: Sender>(mut stack: Stack<S>) -> ! {
        loop {
            let timeout = match stack.update() {
                Ok(t) => t,
                Err(e) => {
                    println!("Error updating stack: {e}");
                    100
                }
            };

            sleep(Duration::from_millis(timeout));
        }
    }

    /// Loop that reads packets from the `serial` line into the stack
    pub fn inbound_loop<S: Sender, R: Read>(mut stack: Stack<S>, serial: R) -> ! {
        let mut reader = FromStd::new(BufReader::new(serial));
        let mut serial_transport = mctp_lib::serial::MctpSerialHandler::new();
        loop {
            let Ok(pkt) = serial_transport
                .recv_sync(&mut reader)
                .inspect_err(|e| println!("Error receiving serial data: {e}"))
            else {
                continue;
            };

            stack
                .inbound(pkt)
                .inspect_err(|e| println!("Error processing inbound packet: {e}"))
                .ok();
        }
    }
}
