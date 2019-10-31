// Copyright 2019 Parity Technologies (UK) Ltd.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

use crate::status::protocol;


use std::{error::Error, io, fmt, num::NonZeroU32, time::Duration};
use std::collections::VecDeque;

use futures::prelude::*;

use libp2p::swarm::{
    KeepAlive,
    SubstreamProtocol,
    ProtocolsHandler,
    ProtocolsHandlerUpgrErr,
    ProtocolsHandlerEvent
};
use libp2p::tokio_io::{AsyncRead, AsyncWrite};

use wasm_timer::{Delay, Instant};

use void::Void;

/// The configuration for outbound requests.
#[derive(Clone, Debug)]
pub struct StatusConfig {
    /// The current status sent on request
    status: protocol::Payload,
    /// The timeout of an outbound request.
    timeout: Duration,
    /// The duration between the last successful outbound or inbound request
    /// and the next outbound request.
    interval: Duration,
    /// The maximum number of failed outbound requests before the associated
    /// connection is deemed unhealthy, indicating to the `Swarm` that it
    /// should be closed.
    max_failures: NonZeroU32,
    /// Whether the connection should generally be kept alive unless
    /// `max_failures` occur.
    keep_alive: bool,
}

impl StatusConfig {
    /// Creates a new `StatusConfig` with the following default settings:
    ///
    ///   * [`StatusConfig::with_interval`] 15s
    ///   * [`StatusConfig::with_timeout`] 20s
    ///   * [`StatusConfig::with_max_failures`] 1
    ///   * [`StatusConfig::with_keep_alive`] false
    ///
    /// These settings have the following effect:
    ///
    ///   * A request is sent every 15 seconds on a healthy connection.
    ///   * Every request sent must yield a response within 20 seconds in order to
    ///     be successful.
    ///   * A single request failure is sufficient for the connection to be subject
    ///     to being closed.
    ///   * The connection may be closed at any time as far as the status protocol
    ///     is concerned, i.e. the status protocol itself does not keep the
    ///     connection alive.
    pub fn new(status: protocol::Payload) -> Self {
        Self {
            status,
            timeout: Duration::from_secs(20),
            interval: Duration::from_secs(15),
            max_failures: NonZeroU32::new(1).expect("1 != 0"),
            keep_alive: false
        }
    }

    /// Sets the request timeout.
    pub fn with_timeout(mut self, d: Duration) -> Self {
        self.timeout = d;
        self
    }

    /// Sets the request interval.
    pub fn with_interval(mut self, d: Duration) -> Self {
        self.interval = d;
        self
    }

    /// Sets the maximum number of consecutive request failures upon which the remote
    /// peer is considered unreachable and the connection closed.
    pub fn with_max_failures(mut self, n: NonZeroU32) -> Self {
        self.max_failures = n;
        self
    }

    /// Sets whether the status protocol itself should keep the connection alive,
    /// apart from the maximum allowed failures.
    ///
    /// By default, the status protocol itself allows the connection to be closed
    /// at any time, i.e. in the absence of request failures the connection lifetime
    /// is determined by other protocol handlers.
    ///
    /// If the maximum  number of allowed request failures is reached, the
    /// connection is always terminated as a result of [`StatusHandler::poll`]
    /// returning an error, regardless of the keep-alive setting.
    pub fn with_keep_alive(mut self, b: bool) -> Self {
        self.keep_alive = b;
        self
    }
}

/// The result of an inbound or outbound request.
pub type StatusResult = Result<StatusSuccess, StatusFailure>;

/// The successful result of exchanging once status.
#[derive(Debug)]
pub enum StatusSuccess {
    /// Received status request
    Requested,
    /// Requested and received status
    Received( protocol::Payload ),
}

/// An outbound request failure.
#[derive(Debug)]
pub enum StatusFailure {
    /// The status request timed out, i.e. no response was received within the
    /// configured timeout.
    Timeout,
    /// The request failed for reasons other than a timeout.
    Other { error: Box<dyn std::error::Error + Send + 'static> }
}

impl fmt::Display for StatusFailure {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            StatusFailure::Timeout => f.write_str("Status timeout"),
            StatusFailure::Other { error } => write!(f, "Status error: {}", error)
        }
    }
}

impl Error for StatusFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            StatusFailure::Timeout => None,
            StatusFailure::Other { error } => Some(&**error)
        }
    }
}

/// Protocol handler that handles requesting the remote at a regular period
/// and answering status requests.
///
/// If the remote doesn't respond, produces an error that closes the connection.
pub struct StatusHandler<TSubstream> {
    /// Configuration options.
    config: StatusConfig,
    /// The timer for when to send the next request.
    next_request: Delay,
    /// The pending results from inbound or outbound requests, ready
    /// to be `poll()`ed.
    pending_results: VecDeque<StatusResult>,
    /// The number of consecutive request failures that occurred.
    failures: u32,
    _marker: std::marker::PhantomData<TSubstream>
}

impl<TSubstream> StatusHandler<TSubstream> {
    /// Builds a new `StatusHandler` with the given configuration.
    pub fn new(config: StatusConfig) -> Self {
        StatusHandler {
            config,
            next_request: Delay::new(Instant::now()),
            pending_results: VecDeque::with_capacity(2),
            failures: 0,
            _marker: std::marker::PhantomData
        }
    }
}

impl<TSubstream> ProtocolsHandler for StatusHandler<TSubstream>
where
    TSubstream: AsyncRead + AsyncWrite,
{
    type InEvent = Void;
    type OutEvent = StatusResult;
    type Error = StatusFailure;
    type Substream = TSubstream;
    type InboundProtocol = protocol::Status;
    type OutboundProtocol = protocol::Status;
    type OutboundOpenInfo = ();

    fn listen_protocol(&self) -> SubstreamProtocol<protocol::Status> {
        SubstreamProtocol::new(protocol::Status( self.config.status ))
    }

    fn inject_fully_negotiated_inbound(&mut self, _: ()) {
        // A request from a remote peer has been answered.
        self.pending_results.push_front(Ok(StatusSuccess::Requested));
    }

    fn inject_fully_negotiated_outbound(&mut self, payload: protocol::Payload, _info: ()) {
        // A request initiated by the local peer was answered by the remote.
        self.pending_results.push_front(Ok(StatusSuccess::Received(payload)));
    }

    fn inject_event(&mut self, _: Void) {}

    fn inject_dial_upgrade_error(&mut self, _info: (), error: ProtocolsHandlerUpgrErr<io::Error>) {
        self.pending_results.push_front(
            Err(match error {
                ProtocolsHandlerUpgrErr::Timeout => StatusFailure::Timeout,
                e => StatusFailure::Other { error: Box::new(e) }
            }))
    }

    fn connection_keep_alive(&self) -> KeepAlive {
        if self.config.keep_alive {
            KeepAlive::Yes
        } else {
            KeepAlive::No
        }
    }

    fn poll(&mut self) -> Poll<ProtocolsHandlerEvent<protocol::Status, (), StatusResult>, Self::Error> {
        if let Some(result) = self.pending_results.pop_back() {
            if let Ok(StatusSuccess::Received ( .. )) = result {
                let next_request = Instant::now() + self.config.interval;
                self.failures = 0;
                self.next_request.reset(next_request);
            }
            if let Err(e) = result {
                self.failures += 1;
                if self.failures >= self.config.max_failures.get() {
                    return Err(e)
                } else {
                    return Ok(Async::Ready(ProtocolsHandlerEvent::Custom(Err(e))))
                }
            }
            return Ok(Async::Ready(ProtocolsHandlerEvent::Custom(result)))
        }

        match self.next_request.poll() {
            Ok(Async::Ready(())) => {
                self.next_request.reset(Instant::now() + self.config.timeout);
                let protocol = SubstreamProtocol::new(protocol::Status( self.config.status ))
                    .with_timeout(self.config.timeout);
                Ok(Async::Ready(ProtocolsHandlerEvent::OutboundSubstreamRequest {
                    protocol,
                    info: (),
                }))
            },
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(e) => Err(StatusFailure::Other { error: Box::new(e) })
        }
    }
}
