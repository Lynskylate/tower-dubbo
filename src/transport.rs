use std::pin::Pin;
use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::atomic::AtomicU64;
use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::task::{Context, Poll};
use tokio::net::TcpStream;
use tokio_util::codec::Framed;
use tokio_tower::Error;
use tower::Service;
use crate::codec::{DubboCodec, DubboMessage};
use crate::error::CodecError;

use tokio_tower::multiplex::{ Client, MultiplexTransport, TagStore};
use tokio_tower::multiplex::client::VecDequePendingStore;



#[derive(Default)]
pub struct CorrelationStore {
    correlation_ids: HashSet<u64>,
    id_gen: AtomicU64,
}

impl TagStore<DubboMessage, DubboMessage> for CorrelationStore {
    type Tag = u64;

    fn assign_tag(self: Pin<&mut Self>, request: &mut DubboMessage) -> u64 {
        let tag = self.id_gen.fetch_add(1, Ordering::SeqCst);
        request.set_id(tag);
        tag
    }

    fn finish_tag(self: Pin<&mut Self>, response: &DubboMessage) -> u64 {
        response.id()
    }
}


type FramedIO<T> = Framed<T, DubboCodec>;

pub type TransportError<T> = Error<MultiplexTransport<FramedIO<T>, CorrelationStore>, DubboMessage>;

pub type TransportClient<T> =
    Client<MultiplexTransport<FramedIO<T>, CorrelationStore>, TransportError<T>, DubboMessage>;

/// Helper for defining a "connection" that provides [`AsyncRead`] and [`AsyncWrite`] for
/// sending messages to Kafka.
pub trait MakeConnection {
    /// The connection type.
    type Connection: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sized;
    /// The error that may be produced when connecting.
    type Error: std::error::Error;
    /// The future used for awaiting a connection.
    type Future: Future<Output = Result<Self::Connection, Self::Error>>;

    /// Check whether a connection is ready to be produced.
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>;

    /// Connect to the connection.
    fn connect(self) -> Self::Future;
}

/// A simple TCP connection.
pub struct TcpConnection {
    addr: SocketAddr,
}

impl TcpConnection {
    /// Create a new connection using the provided socket address.
    pub fn new(addr: SocketAddr) -> Self {
        Self { addr }
    }
}

impl MakeConnection for TcpConnection {
    type Connection = TcpStream;
    type Error = io::Error;
    type Future =
        Pin<Box<dyn Future<Output = io::Result<Self::Connection>> + Send + Sync + 'static>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn connect(self) -> Self::Future {
        Box::pin(async move {
            let tcp_stream = TcpStream::connect(self.addr).await?;
            Ok(tcp_stream)
        })
    }
}

/// Helper for building new clients.
pub struct MakeClient<C> {
    connection: C,
}

impl<C> MakeClient<C>
where
    C: MakeConnection + 'static,
{
    /// Create a new [`MakeClient`] with the provided connection.
    pub fn with_connection(connection: C) -> Self {
        Self { connection }
    }

    /// Wait for the connection and produce a new client instance when ready.
    pub async fn into_client(self) -> Result<TransportClient<C::Connection>, C::Error> {
        let io = self.connection.connect().await?;
        let io = Framed::new(io, DubboCodec::new());

        let client = Client::builder(MultiplexTransport::new(io, CorrelationStore::default()))
            .pending_store(VecDequePendingStore::default())
            .build();

        Ok(client)
    }
}

