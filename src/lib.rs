use crate::codec::DubboCodec;

use std::error::Error;
use std::io;

use futures::StreamExt;
use hessian_rs::de::Deserializer;
use hessian_rs::value::Value;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};

pub mod codec;
pub mod constant;
pub mod error;

use tokio_util::codec::{Decoder, Framed, FramedRead, FramedWrite};

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