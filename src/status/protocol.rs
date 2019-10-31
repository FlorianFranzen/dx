// Copyright 2018 Parity Technologies (UK) Ltd.
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

use std::{io, iter};

use futures::{prelude::*, future, try_ready};

use libp2p::core::{InboundUpgrade, OutboundUpgrade, UpgradeInfo, upgrade::Negotiated};
use libp2p::tokio_io::{io as nio, AsyncRead, AsyncWrite};


/// Payload type of exchanged status information
pub type Payload = [u8; 20];

/// Represents a prototype for an upgrade to handle the status protocol.
///
/// In this preliminary implementation the status is made up of a 20 bytes
/// of data representing a git revision (i.e. a SHA-1 hash) that is only
/// send unidirectional.
///
/// The protocol works the following way:
///
/// - Dialer sends status request.
/// - Listener receives request and sends back status.
/// - Dialer receives the data and returns it via event.
///
/// The dialer produces a 20-byte array, which corresponds to the received payload.
#[derive(Default, Debug, Copy, Clone)]
pub struct Status ( pub Payload );

impl UpgradeInfo for Status {
    type Info = &'static [u8];
    type InfoIter = iter::Once<Self::Info>;

    fn protocol_info(&self) -> Self::InfoIter {
        iter::once(b"/dx/status/0.1.0")
    }
}

type SendStatus<T> = nio::WriteAll<Negotiated<T>, Payload>;
type Flush<T> = nio::Flush<Negotiated<T>>;
type Shutdown<T> = nio::Shutdown<Negotiated<T>>;

impl<TSocket> InboundUpgrade<TSocket> for Status
where
    TSocket: AsyncRead + AsyncWrite,
{
    type Output = ();
    type Error = io::Error;
    type Future = future::Map<
        future::AndThen<
        future::AndThen<
            SendStatus<TSocket>,
            Flush<TSocket>, fn((Negotiated<TSocket>, Payload)) -> Flush<TSocket>>,
            Shutdown<TSocket>, fn(Negotiated<TSocket>) -> Shutdown<TSocket>>,
    fn(Negotiated<TSocket>) -> ()>;

    #[inline]
    fn upgrade_inbound(self, socket: Negotiated<TSocket>, _: Self::Info) -> Self::Future {
        nio::write_all(socket, self.0)
            .and_then::<fn(_) -> _, _>(|(sock, _)| nio::flush(sock))
            .and_then::<fn(_) -> _, _>(|sock| nio::shutdown(sock))
            .map(|_| ())
    }
}

impl<TSocket> OutboundUpgrade<TSocket> for Status
where
    TSocket: AsyncRead + AsyncWrite,
{
    type Output = Payload;
    type Error = io::Error;
    type Future = StatusDialer<Negotiated<TSocket>>;

    #[inline]
    fn upgrade_outbound(self, socket: Negotiated<TSocket>, _: Self::Info) -> Self::Future {
        StatusDialer {
            state: StatusDialerState::Read {
                inner: nio::read_exact(socket, [0; 20]),
            },
        }
    }
}

/// A `StatusDialer` is a future that expects to receive the current status.
pub struct StatusDialer<TSocket> {
    state: StatusDialerState<TSocket>
}

enum StatusDialerState<TSocket> {
   Read {
       inner: nio::ReadExact<TSocket, Payload>,
    },
    Shutdown {
        inner: nio::Shutdown<TSocket>,
        payload: Payload,
    },
}

impl<TSocket> Future for StatusDialer<TSocket>
where
    TSocket: AsyncRead + AsyncWrite,
{
    type Item = Payload;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            self.state = match self.state {
                StatusDialerState::Read { ref mut inner } => {
                    let (socket, payload) = try_ready!(inner.poll());
                    StatusDialerState::Shutdown {
                        inner: nio::shutdown(socket),
                        payload,
                    }
                },
                StatusDialerState::Shutdown { ref mut inner, payload } => {
                    try_ready!(inner.poll());
                    return Ok(Async::Ready(payload));
                },
            }
        }
    }
}
