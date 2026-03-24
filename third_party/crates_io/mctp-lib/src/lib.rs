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

//! This crate provides a platform-agnostic MCTP stack.
//!
//! It uses the [mctp-estack](https://docs.rs/mctp-estack/latest/mctp_estack/) and re-exports most
//! parts of it.
#![cfg_attr(not(test), no_std)]
#![deny(unsafe_code)]
#![deny(missing_docs)]
#![deny(clippy::missing_panics_doc)]
#![deny(clippy::panic)]
#![deny(clippy::panicking_overflow_checks)]
#![deny(clippy::indexing_slicing)]

use mctp::{Eid, Error, MsgIC, MsgType, Result, Tag};

use mctp_estack::fragment::Fragmenter;
pub use mctp_estack::*;

#[derive(Debug)]
struct ReqHandle {
    /// Destination EID
    eid: Eid,
    /// Tag from last send operation
    ///
    /// Has to be cleared upon receiving a response.
    // A no-expire option might be added as a future improvement.
    last_tag: Option<Tag>,
}
impl ReqHandle {
    fn new(eid: Eid) -> ReqHandle {
        ReqHandle {
            eid,
            last_tag: None,
        }
    }
}

/// A platform-agnostic MCTP stack with routing
///
/// Only a single port/bus is supported
#[derive(Debug)]
pub struct Router<S: Sender, const MAX_LISTENER_HANDLES: usize, const MAX_REQ_HANDLES: usize> {
    stack: Stack,
    sender: S,
    /// Listener handles
    ///
    /// The index is used to construct the AppCookie.
    listeners: [Option<MsgType>; MAX_LISTENER_HANDLES],
    /// Request handles
    ///
    /// The index is used to construct the AppCookie.
    requests: [Option<ReqHandle>; MAX_REQ_HANDLES],
}

impl<S: Sender, const MAX_LISTENER_HANDLES: usize, const MAX_REQ_HANDLES: usize>
    Router<S, MAX_LISTENER_HANDLES, MAX_REQ_HANDLES>
{
    /// Create a new `Router` that routes `outbound` trafic to [S](Sender)
    pub fn new(own_eid: Eid, now_millis: u64, outbound: S) -> Self {
        let stack = Stack::new(own_eid, now_millis);
        Router {
            stack,
            sender: outbound,
            listeners: [None; MAX_LISTENER_HANDLES],
            requests: [const { None }; MAX_REQ_HANDLES],
        }
    }

    /// Update the stack
    ///
    /// Returns an interval value in milliseconds in which the next call to `update()` should be
    /// issued.
    ///
    /// Note:
    /// It is the obligation of the implementer to wake up expired receive calls. However,
    /// this may be changed in future versions.
    pub fn update(&mut self, now_millis: u64) -> Result<u64> {
        self.stack.update(now_millis).map(|x| x.0)
    }

    /// Provide an incoming packet to the router.
    ///
    /// This expects a single MCTP packet, without a transport binding header.
    ///
    /// Returns `Ok(Some(AppCookie))` for a associated listener or request,
    /// or `Ok(None)` if the message was discarded.
    pub fn inbound(&mut self, pkt: &[u8]) -> Result<Option<AppCookie>> {
        let own_eid = self.stack.eid();
        let Some(mut msg) = self.stack.receive(pkt)? else {
            return Ok(None);
        };

        if msg.dest != own_eid && msg.dest != Eid(0) {
            // Drop messages if eid does not match (for now).
            // EID 0 messages are used for physical addressing
            // and will thus be processed.
            return Ok(None);
        }

        match msg.tag {
            Tag::Unowned(_) => {
                // check for matching requests
                if let Some(cookie) = msg.cookie()
                    && Self::requests_index_from_cookie(cookie)
                        .is_some_and(|i| self.requests.get(i).is_some_and(|r| r.is_some()))
                {
                    msg.retain();
                    return Ok(Some(cookie));
                }
                // In this case an unowned message not associated with a request was received.
                // This might happen if this endpoint was intended to route the packet to a different
                // bus it is connected to (bridge configuration).
                // Support for this is missing right now.
            }
            Tag::Owned(_) => {
                // check for matching listeners and retain with cookie
                for i in 0..self.listeners.len() {
                    if self.listeners.get(i).ok_or(Error::InternalError)? == &Some(msg.typ) {
                        let cookie = Some(Self::listener_cookie_from_index(i));
                        msg.set_cookie(cookie);
                        msg.retain();
                        return Ok(cookie);
                    }
                }
            }
        }

        // Return Ok(()) even if a message has been discarded
        Ok(None)
    }

    /// Allocate a new request "_Handle_"
    pub fn req(&mut self, eid: Eid) -> Result<AppCookie> {
        for (index, handle) in self.requests.iter_mut().enumerate() {
            if handle.is_none() {
                let _ = handle.insert(ReqHandle::new(eid));
                return Ok(Self::req_cookie_from_index(index));
            }
        }
        Err(mctp::Error::NoSpace)
    }

    /// Allocate a new listener for [`typ`](MsgType)
    ///
    /// Returns an [AppCookie] when successful, [AddrInUse](mctp::Error::AddrInUse) when a listener
    /// for `typ` already exists,
    /// [NoSpace](mctp::Error::NoSpace) when all listener slots are occupied.
    pub fn listener(&mut self, typ: MsgType) -> Result<AppCookie> {
        if self.listeners.iter().any(|x| x == &Some(typ)) {
            return Err(mctp::Error::AddrInUse);
        }
        for (index, handle) in self.listeners.iter_mut().enumerate() {
            if handle.is_none() {
                let _ = handle.insert(typ);
                return Ok(Self::listener_cookie_from_index(index));
            }
        }
        Err(mctp::Error::NoSpace)
    }

    /// Get the currently configured _Eid_ for this endpoint
    pub fn get_eid(&self) -> Eid {
        self.stack.eid()
    }

    /// Set the _Eid_ for this endpoint
    pub fn set_eid(&mut self, eid: Eid) -> Result<()> {
        self.stack.set_eid(eid.0)
    }

    /// Send a message
    ///
    /// When responding to a request received by a listener, `eid` and `tag` have to be set.
    /// A request usually won't set an ` eid `.
    /// When no `tag` is supplied for a request, a new one will be allocated.
    pub fn send(
        &mut self,
        eid: Option<Eid>,
        typ: MsgType,
        tag: Option<Tag>,
        ic: MsgIC,
        cookie: AppCookie,
        buf: &[u8],
    ) -> Result<Tag> {
        self.send_vectored(eid, typ, tag, ic, cookie, &[buf])
    }

    /// Send a vectored message
    ///
    /// When responding to a request received by a listener, `eid` and `tag` have to be set.
    /// A request usually won't set an `eid`.
    /// When no `tag` is supplied for a request, a new one will be allocated.
    ///
    /// The `bufs` will be copied to a new buffer with `MAX_PAYLOAD` size.
    pub fn send_vectored(
        &mut self,
        eid: Option<Eid>,
        typ: MsgType,
        tag: Option<Tag>,
        ic: MsgIC,
        cookie: AppCookie,
        bufs: &[&[u8]],
    ) -> Result<Tag> {
        let Some(eid) = eid.or(self.lookup_request(cookie).map(|r| r.eid)) else {
            return Err(Error::InvalidInput);
        };
        let frag = self.stack.start_send(
            eid,
            typ,
            tag,
            true,
            ic,
            Some(self.sender.get_mtu()),
            Some(cookie),
        )?;

        self.sender.send_vectored(eid, frag, bufs)
    }

    /// Receive a message associated with a [`AppCookie`]
    ///
    /// Returns `None` when no message is available for the listener/request.
    ///
    /// The message can be retained and received at a later point again (see [MctpMessage::retain()]).
    pub fn recv(&mut self, cookie: AppCookie) -> Option<mctp_estack::MctpMessage<'_>> {
        self.stack.get_deferred_bycookie(&[cookie])
    }

    /// Unbind a listener/request
    ///
    /// This has to be called to free the request/listener slot.
    /// Returns [BadArgument](Error::BadArgument) for cookies that are malformed or non-existent.
    pub fn unbind(&mut self, cookie: AppCookie) -> Result<()> {
        if Self::cookie_is_listener(&cookie) {
            self.listeners
                .get_mut(Self::listeners_index_from_cookie(cookie).ok_or(Error::BadArgument)?)
                .ok_or(Error::InternalError)?
                .take()
                .ok_or(Error::BadArgument)?;
            Ok(())
        } else {
            let req = self
                .requests
                .get_mut(Self::requests_index_from_cookie(cookie).ok_or(Error::BadArgument)?)
                .ok_or(Error::InternalError)?
                .take()
                .ok_or(Error::BadArgument)?;
            if let ReqHandle {
                eid,
                last_tag: Some(tag),
            } = req
            {
                self.stack.cancel_flow(eid, tag.tag());
            }
            Ok(())
        }
    }

    fn lookup_request(&self, cookie: AppCookie) -> Option<&ReqHandle> {
        Self::requests_index_from_cookie(cookie)
            .and_then(|i| self.requests.get(i).and_then(|r| r.as_ref()))
    }

    /// Function to create a router unique AppCookie for listeners
    ///
    /// Currently, the listeners are just the index ranging from 0 to LISTENER_HANDLES-1.
    /// Requests are enumerated from LISTENER_HANDLES to LISTENER_HANDLES+REQUEST_HANDLES-1
    fn listener_cookie_from_index(i: usize) -> AppCookie {
        debug_assert!(
            i < MAX_LISTENER_HANDLES,
            "tried to create out of range listener AppCookie!"
        );
        AppCookie(i)
    }

    /// Function to create a router unique [AppCookie] for requests
    ///
    /// Currently, the listeners are just the index ranging from 0 to `LISTENER_HANDLES-1`.
    /// Requests are enumerated from `LISTENER_HANDLES` to `LISTENER_HANDLES+REQUEST_HANDLES-1`.
    fn req_cookie_from_index(i: usize) -> AppCookie {
        debug_assert!(
            i < MAX_REQ_HANDLES,
            "tried to create out of range request AppCookie!"
        );
        AppCookie(i + MAX_LISTENER_HANDLES)
    }

    /// Get the listener array index from an [AppCookie]
    ///
    /// Returns `None` for invalid cookies.
    fn listeners_index_from_cookie(cookie: AppCookie) -> Option<usize> {
        if cookie.0 < MAX_LISTENER_HANDLES {
            Some(cookie.0)
        } else {
            None
        }
    }

    /// Get the requester array index from a [AppCookie]
    ///
    /// Returns `None` for invalid cookies.
    fn requests_index_from_cookie(cookie: AppCookie) -> Option<usize> {
        if cookie.0 >= MAX_LISTENER_HANDLES && cookie.0 < (MAX_LISTENER_HANDLES + MAX_REQ_HANDLES) {
            Some(cookie.0 - MAX_LISTENER_HANDLES)
        } else {
            None
        }
    }

    /// Check if a cookie is a corresponding to a listener
    ///
    /// Checks based on the contained id.
    /// Returns false for request cookies.
    fn cookie_is_listener(cookie: &AppCookie) -> bool {
        cookie.0 < MAX_LISTENER_HANDLES
    }
}

/// A Sender used by a [Router] to send data
///
/// Implemented by a transport binding for sending packets.
pub trait Sender {
    /// Send a packet fragmented by `fragmenter` with the payload `payload`
    fn send_vectored(&mut self, eid: Eid, fragmenter: Fragmenter, payload: &[&[u8]])
    -> Result<Tag>;
    /// Get the MTU of a MCTP packet fragment (without transport headers)
    fn get_mtu(&self) -> usize;
}

#[cfg(test)]
#[allow(clippy::panic)]
mod test {
    use core::cell::RefCell;

    use mctp::Eid;

    use crate::{Router, Sender};

    struct DoNothingSender;

    impl Sender for DoNothingSender {
        fn send_vectored(
            &mut self,
            _eid: Eid,
            fragmenter: mctp_estack::fragment::Fragmenter,
            payload: &[&[u8]],
        ) -> core::result::Result<mctp::Tag, mctp::Error> {
            let _ = payload;
            Ok(fragmenter.tag())
        }

        fn get_mtu(&self) -> usize {
            255
        }
    }

    struct BufferSender<'a, const MTU: usize> {
        packets: &'a RefCell<Vec<Vec<u8>>>,
    }

    impl<const MTU: usize> Sender for BufferSender<'_, MTU> {
        fn send_vectored(
            &mut self,
            _eid: Eid,
            mut fragmenter: mctp_estack::fragment::Fragmenter,
            payload: &[&[u8]],
        ) -> core::result::Result<mctp::Tag, mctp::Error> {
            loop {
                let mut buf = [0; MTU];
                match fragmenter.fragment_vectored(payload, &mut buf) {
                    mctp_estack::fragment::SendOutput::Packet(items) => {
                        self.packets.borrow_mut().push(items.into())
                    }
                    mctp_estack::fragment::SendOutput::Complete { tag, cookie: _ } => {
                        return Ok(tag);
                    }
                    mctp_estack::fragment::SendOutput::Error { err, cookie: _ } => return Err(err),
                }
            }
        }

        fn get_mtu(&self) -> usize {
            MTU
        }
    }

    /// Test the creation of request and listener handles (`AppCookies`)
    #[test]
    fn test_handle_creation() {
        const REQ_HANDLES: usize = 8;
        const LISTENER_HANDLES: usize = 8;
        let outbound = DoNothingSender;
        let mut router: Router<_, REQ_HANDLES, LISTENER_HANDLES> =
            Router::new(Eid(42), 0, outbound);

        // create a new listener and expect the cookie value to be 0 (raw index of the underlying table)
        let listener = router.listener(mctp::MsgType(0));
        assert!(listener.is_ok());
        assert!(listener.as_ref().is_ok_and(|x| x.0 == 0));

        // create a new request
        // we expect the value to be MAX_LISTENER_HANDLES (request table index 0 + offset)
        let req = router.req(Eid(112));
        assert!(req.is_ok());
        assert!(req.as_ref().is_ok_and(|x| x.0 == LISTENER_HANDLES));

        router
            .unbind(listener.unwrap())
            .expect("failed to unbind listener handle");
        router
            .unbind(req.unwrap())
            .expect("failed to unbind request handle");
        for listener in router.listeners {
            assert!(listener.is_none());
        }
        for request in router.requests {
            assert!(request.is_none());
        }
    }

    /// Create two routers, send a request from B to A and receive the echo response
    #[test]
    fn roundtrip() {
        const REQ_HANDLES: usize = 8;
        const LISTENER_HANDLES: usize = 8;
        let buf_out_a = RefCell::new(Vec::new());
        let outbound_a: BufferSender<255> = BufferSender {
            packets: &buf_out_a,
        };
        let mut router_a: Router<_, LISTENER_HANDLES, REQ_HANDLES> =
            Router::new(Eid(42), 0, outbound_a);

        let buf_out_b = RefCell::new(Vec::new());
        let outbound_b: BufferSender<255> = BufferSender {
            packets: &buf_out_b,
        };
        let mut router_b: Router<_, LISTENER_HANDLES, REQ_HANDLES> =
            Router::new(Eid(112), 0, outbound_b);

        // create a new listener and expect the cookie value to be 0 (raw index of the underlying table)
        let listener = router_a.listener(mctp::MsgType(0)).unwrap();

        let requester = router_b.req(Eid(42)).unwrap();

        let payload = [1; 300]; // 300 byte payload to exceed 255 byte MTU
        router_b
            .send(
                None,
                mctp::MsgType(0),
                None,
                mctp::MsgIC(false),
                requester,
                &payload,
            )
            .unwrap();

        let packets = buf_out_b.borrow();
        for pkt in packets.as_slice() {
            router_a.inbound(pkt).unwrap();
        }

        let message = router_a.recv(listener).unwrap();
        let (msg_source, msg_typ, msg_tag, msg_ic) =
            (message.source, message.typ, message.tag, message.ic);
        let msg_payload: Vec<_> = message.payload.into();
        drop(message);

        assert_eq!(
            &msg_payload, &payload,
            "Received payload does not match send payload"
        );
        assert!(
            msg_tag.is_owner(),
            "Received message is not a request (tag is unowned)"
        );

        router_a
            .send(
                Some(msg_source),
                msg_typ,
                Some(super::Tag::Unowned(msg_tag.tag())),
                msg_ic,
                listener,
                &msg_payload,
            )
            .unwrap();

        let packets = buf_out_a.borrow();
        for pkt in packets.as_slice() {
            router_b.inbound(pkt).unwrap();
        }

        let message = router_b.recv(requester).unwrap();
        assert_eq!(
            message.payload, &payload,
            "Received payload does not match send payload"
        );
        assert!(
            !message.tag.is_owner(),
            "Received message is not a response (tag is unowned)"
        );
    }
}
