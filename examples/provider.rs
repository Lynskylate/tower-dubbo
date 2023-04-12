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
    todo!()
}

