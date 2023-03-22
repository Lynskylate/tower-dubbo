use crate::codec::DubboHeader;
use crate::codec::{DubboCodec, DubboMessage, RequestInfoBuilder};
use crate::conn::{MakeConnection, TcpConnection};
use std::collections::HashSet;
use std::pin::Pin;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use tokio_tower::Error;
use tokio_util::codec::Framed;
use tower::Service;

use tokio_tower::multiplex::client::VecDequePendingStore;
use tokio_tower::multiplex::{Client, MultiplexTransport, TagStore};

#[derive(Default)]
pub struct CorrelationStore {
    correlation_ids: HashSet<u64>,
    id_gen: AtomicU64,
}

impl TagStore<DubboMessage, DubboMessage> for CorrelationStore {
    type Tag = u64;

    fn assign_tag(mut self: Pin<&mut Self>, request: &mut DubboMessage) -> u64 {
        let tag = self.id_gen.fetch_add(1, Ordering::SeqCst);
        self.correlation_ids.insert(request.id());
        request.set_id(tag);
        tag
    }

    fn finish_tag(mut self: Pin<&mut Self>, response: &DubboMessage) -> u64 {
        self.correlation_ids.remove(&response.id());
        response.id()
    }
}

type FramedIO<T> = Framed<T, DubboCodec>;

pub type TransportError<T> = Error<MultiplexTransport<FramedIO<T>, CorrelationStore>, DubboMessage>;

pub type TransportClient<T> =
    Client<MultiplexTransport<FramedIO<T>, CorrelationStore>, TransportError<T>, DubboMessage>;

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

mod tests {

    use std::collections::HashMap;

    use super::*;
    use hessian_rs::{value::ToHessian, Value};
    use tower::ServiceExt;

    fn build_req() -> DubboMessage {
        let header = DubboHeader::default();
        let map = {
            let mut gender: HashMap<Value, Value> = HashMap::new();
            gender.insert("name".to_hessian(), "MAN".to_hessian());

            let mut map = HashMap::new();
            map.insert(
                "sex".to_hessian(),
                Value::Map(("org.apache.dubbo.sample.Gender", gender).into()),
            );
            map.insert("name".to_hessian(), "".to_hessian());
            map.insert("id".to_hessian(), "003".to_hessian());
            map.insert("time".to_hessian(), Value::Null);
            map.insert("age".to_hessian(), 0.to_hessian());
            map
        };

        let attachments = {
            let mut attachments = HashMap::new();
            attachments.insert("path".into(), "org.apache.dubbo.sample.UserProvider".into());
            attachments.insert(
                "interface".into(),
                "org.apache.dubbo.sample.UserProvider".into(),
            );
            attachments.insert("enviroment".into(), "dev".into());
            attachments.insert("timeout".into(), "0".into());
            attachments.insert("version".into(), "".into());
            attachments
        };

        let body = RequestInfoBuilder::default()
            .service_name("org.apache.dubbo.sample.UserProvider".into())
            .method_name("GetUser".into())
            .version("1.0.2".into())
            .service_version("".into())
            .method_paramter_type(vec!["org.apache.dubbo.sample.User".into()])
            .method_arguments(vec![Value::Map(
                ("org.apache.dubbo.sample.User", map).into(),
            )])
            .attachments(attachments)
            .build()
            .unwrap();

        DubboMessage::with_request(header, body)
    }
    #[tokio::test]
    async fn test_client() {
        let addr = "127.0.0.1:20000".parse().unwrap();
        let conn = TcpConnection::new(addr);
        let mut client = MakeClient::with_connection(conn)
            .into_client()
            .await
            .unwrap();

        client.ready().await.unwrap();
        let resp = client.call(build_req()).await.unwrap();
        println!("{:?}", resp);
    }
}
