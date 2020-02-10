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

use futures::{future::BoxFuture, prelude::*};

use libp2p::core::{InboundUpgrade, OutboundUpgrade, UpgradeInfo};


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


impl<TSocket> InboundUpgrade<TSocket> for Status
where
    TSocket: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    type Output = ();
    type Error = io::Error;
    type Future = BoxFuture<'static, Result<Self::Output, Self::Error>>;

    fn upgrade_inbound(self, mut socket: TSocket, _: Self::Info) -> Self::Future {
        async move {
            socket.write_all(&self.0).await?;
            socket.flush().await?;
            Ok(())
        }.boxed()
    }
}

impl<TSocket> OutboundUpgrade<TSocket> for Status
where
    TSocket: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    type Output = Payload;
    type Error = io::Error;
    type Future = BoxFuture<'static, Result<Self::Output, Self::Error>>;

    fn upgrade_outbound(self, mut socket: TSocket, _: Self::Info) -> Self::Future {
        async move {
            let mut payload = [0u8; 20];
            socket.read_exact(&mut payload).await?;
            Ok(payload)
        }.boxed()
    }
}

