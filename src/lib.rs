use crate::codec::DubboCodec;

use std::error::Error;
use std::io;

use futures::StreamExt;
use hessian_rs::de::Deserializer;
use hessian_rs::value::Value;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};


pub mod error;
pub mod codec;
pub mod constant;



use tokio_util::codec::{Framed, FramedRead, FramedWrite, Decoder};




async fn process(stream: TcpStream) -> Result<(), Box<dyn Error>> {
    let mut transport = Framed::new(stream, DubboCodec::new());

    while let Some(rawFrame) = transport.next().await {
        match rawFrame {
            Ok(frame) => {
                println!("{:?}", &frame);
                // println!("{:?}", &frame[..]);
                // let response = respond(request).await?;
                // transport.send(response).await?;
            }
            Err(e) => return Err(e.into()),
        }
    }

    Ok(())
}

// #[tokio::main]
// async fn main() -> Result<(), Box<dyn Error>> {
//     // Parse the arguments, bind the TCP socket we'll be listening to, spin up
//     // our worker threads, and start shipping sockets to those worker threads.
//     let addr = env::args()
//         .nth(1)
//         .unwrap_or_else(|| "127.0.0.1:20000".to_string());
//     let server = TcpListener::bind(&addr).await?;
//     println!("Listening on: {}", addr);

//     loop {
//         let (stream, _) = server.accept().await?;
//         tokio::spawn(async move {
//             if let Err(e) = process(stream).await {
//                 println!("failed to process connection; error = {}", e);
//             }
//         });
//     }
// }
#[cfg(test)]
mod tests {
    use crate::codec::DubboCodec;
    use bytes::{BytesMut, BufMut};
    use tokio_util::codec::Decoder;
#[test]
fn test_codec() {
    let dubbo_request_raw_bytes = &[
        0xda, 0xbb, 0xc2, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x01,
        b'G', 0x05, 0x32, 0x2e, 0x30, 0x2e, 0x32, 0x30, 0x24, b'o', b'r', b'g', 0x2e, b'a', b'p',
        b'a', b'c', b'h', b'e', 0x2e, b'd', b'u', b'b', b'b', b'o', 0x2e, b's', b'a', b'm', b'p',
        b'l', b'e', 0x2e, b'U', b's', b'e', b'r', b'P', b'r', b'o', b'v', b'i', b'd', b'e', b'r',
        0x00, 0x07, b'G', b'e', b't', b'U', b's', b'e', b'r', 0x1e, b'L', b'o', b'r', b'g', 0x2f,
        b'a', b'p', b'a', b'c', b'h', b'e', 0x2f, b'd', b'u', b'b', b'b', b'o', 0x2f, b's', b'a',
        b'm', b'p', b'l', b'e', 0x2f, b'U', b's', b'e', b'r', 0x3b, b'C', 0x1c, b'o', b'r', b'g',
        0x2e, b'a', b'p', b'a', b'c', b'h', b'e', 0x2e, b'd', b'u', b'b', b'b', b'o', 0x2e, b's',
        b'a', b'm', b'p', b'l', b'e', 0x2e, b'U', b's', b'e', b'r', 0x95, 0x02, b'i', b'd', 0x04,
        b'n', b'a', b'm', b'e', 0x03, b'a', b'g', b'e', 0x04, b't', b'i', b'm', b'e', 0x03, b's',
        b'e', b'x', 0x60, 0x03, 0x30, 0x30, 0x33, 0x00, 0x90, b'N', b'C', 0x1e, b'o', b'r', b'g',
        0x2e, b'a', b'p', b'a', b'c', b'h', b'e', 0x2e, b'd', b'u', b'b', b'b', b'o', 0x2e, b's',
        b'a', b'm', b'p', b'l', b'e', 0x2e, b'G', b'e', b'n', b'd', b'e', b'r', 0x91, 0x04, b'n',
        b'a', b'm', b'e', b'a', 0x03, b'M', b'A', b'N', b'H', 0x09, b'i', b'n', b't', b'e', b'r',
        b'f', b'a', b'c', b'e', 0x30, 0x24, b'o', b'r', b'g', 0x2e, b'a', b'p', b'a', b'c', b'h',
        b'e', 0x2e, b'd', b'u', b'b', b'b', b'o', 0x2e, b's', b'a', b'm', b'p', b'l', b'e', 0x2e,
        b'U', b's', b'e', b'r', b'P', b'r', b'o', b'v', b'i', b'd', b'e', b'r', 0x07, b't', b'i',
        b'm', b'e', b'o', b'u', b't', 0x01, 0x30, 0x07, b'v', b'e', b'r', b's', b'i', b'o', b'n',
        0x00, 0x05, b'a', b's', b'y', b'n', b'c', 0x05, b'f', b'a', b'l', b's', b'e', 0x0b, b'e',
        b'n', b'v', b'i', b'r', b'o', b'n', b'm', b'e', b'n', b't', 0x03, b'd', b'e', b'v', 0x04,
        b'p', b'a', b't', b'h', 0x30, 0x24, b'o', b'r', b'g', 0x2e, b'a', b'p', b'a', b'c', b'h',
        b'e', 0x2e, b'd', b'u', b'b', b'b', b'o', 0x2e, b's', b'a', b'm', b'p', b'l', b'e', 0x2e,
        b'U', b's', b'e', b'r', b'P', b'r', b'o', b'v', b'i', b'd', b'e', b'r', b'Z',
    ];
    let mut buf = BytesMut::new();
    buf.put_slice(dubbo_request_raw_bytes);
    let mut codec = DubboCodec::new();
    let res = codec.decode(&mut buf).unwrap().unwrap();
    // assert_eq!(dubbo_request_raw_bytes.len(), res.len());
    // assert_eq!(dubbo_request_raw_bytes, &res[..]);
    // let body = &res[16..];

    // let mut de = Deserializer::new(body);

    // let dubbo_version = de.read_value().unwrap();
    // let dubbo_service_name = de.read_value().unwrap();
    // let service_version = de.read_value().unwrap();
    // let method_name = de.read_value().unwrap();
    // let method_parameter_types = de.read_value().unwrap();
    // let parameters = de.read_value().unwrap();
    // let attachments = de.read_value().unwrap();

    // println!("{:?}, {:?}, {:?}, {:?}, {:?}", service_version, method_name, method_parameter_types, parameters, attachments);
    // assert_eq!(dubbo_version, Value::String("2.0.2".into()));
    // assert_eq!(dubbo_service_name, Value::String("org.apache.dubbo.sample.UserProvider".into()));
    // assert_eq!(service_version, Value::String("".into()));
    // assert_eq!(method_name, Value::String("GetUser".into()));
    // assert_eq!(method_parameter_types, Value::String("Lorg/apache/dubbo/sample/user;".into()));

}

}