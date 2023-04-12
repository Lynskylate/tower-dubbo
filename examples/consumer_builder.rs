use std::collections::HashMap;
use std::time::Duration;
use hessian_rs::Value;
use hessian_rs::value::ToHessian;
use tower::{ServiceBuilder, ServiceExt};
use tower_service::Service;
use tower_dubbo::codec::{DubboHeader, DubboMessage, RequestInfoBuilder};
use tower_dubbo::conn::TcpConnection;
use tower_dubbo::transport::MakeClient;

#[tokio::main]
async fn main() {
    let addr = "127.0.0.1:20000".parse().unwrap();
    let conn = TcpConnection::new(addr);
    let mut service = MakeClient::with_connection(conn)
        .into_client()
        .await
        .unwrap();
    let mut client = ServiceBuilder::new()
        // .buffer(5)
        // .concurrency_limit(3)
        .rate_limit(1, Duration::from_secs(1))
        .service(service);
    for _ in 0..10 {
        client.ready().await.unwrap();
        let resp = client.call(build_req()).await.unwrap();
        println!("{:?}", resp);
    }

}


fn build_req() -> DubboMessage {
    let header = DubboHeader::default();
    let map = {
        let mut gender: HashMap<Value, Value> = HashMap::new();
        gender.insert("name".to_hessian(), "MAN".to_hessian());

        let mut map = HashMap::new();
        // map.insert(
        //     "sex".to_hessian(),
        //     Value::Map(("org.apache.dubbo.sample.Gender", gender).into()),
        // );
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