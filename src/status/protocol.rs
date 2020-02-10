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

#[cfg(test)]
mod tests {
    use super::Status;
    use crate::status::generate_payload;
    use futures::prelude::*;
    use libp2p::core::{
        upgrade,
        multiaddr::multiaddr,
        transport::{
            Transport,
            ListenerEvent,
            memory::MemoryTransport
        }
    };
    use rand::{thread_rng, Rng};
    use std::time::Duration;

    #[test]
    fn status_send_recv() {
        let mem_addr = multiaddr![Memory(thread_rng().gen::<u64>())];
        let mut listener = MemoryTransport.listen_on(mem_addr).unwrap();

        let listener_addr =
            if let Some(Some(Ok(ListenerEvent::NewAddress(a)))) = listener.next().now_or_never() {
                a
            } else {
                panic!("MemoryTransport not listening on an address!");
            };

        let payload = generate_payload();

        async_std::task::spawn(async move {
            let listener_event = listener.next().await.unwrap();
            let (listener_upgrade, _) = listener_event.unwrap().into_upgrade().unwrap();
            let conn = listener_upgrade.await.unwrap();
            upgrade::apply_inbound(conn, Status(payload)).await.unwrap();
        });

        async_std::task::block_on(async move {
            let c = MemoryTransport.dial(listener_addr).unwrap().await.unwrap();
            let received = upgrade::apply_outbound(c, Status([0u8; 20]), upgrade::Version::V1).await.unwrap();
            assert!(received == payload);
        });
    }
}
