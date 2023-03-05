// document: https://dubbo.apache.org/zh-cn/blog/2018/10/05/dubbo-%E5%8D%8F%E8%AE%AE%E8%AF%A6%E8%A7%A3/#codec%E7%9A%84%E5%AE%9A%E4%B9%89
use crate::error::CodecError;
use crate::constant::{DEFAULT_HEAD_SIZE, DEFAULT_MAX_MESSAGE_SIZE};

use std::any::Any;
use std::collections::HashMap;

use tokio_util::codec::{self, Decoder, Encoder};
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use bytes::{BytesMut, Buf};



pub trait Serializer {
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MessageType {
    Response = 0x0,
    Request = 0x1,
}


#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum SerializationType {
    Hessian2 = 2,
    Json = 3,
    MsgPack = 4,
    Protobuf = 21,
}


#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum MessageStatus {
    Ok = 20,
    ClientTimeout = 30,
    ServerTimeout = 31,
    BadRequest = 40,
    BadResponse = 50,
    ServiceNotFound = 60,
    ServiceError = 70,
    ServerError = 80,
    ClientError = 90,
    ServerThreadPoolExhastedError = 100,
    Unknow(u8),
}


#[derive(Debug, Clone, Copy)]
pub struct DubboHeader {
    // 1 bit Indivate whether the message is request or response
    msg_type: MessageType,
    // 1bit Indicate whether the message is two way, if is false, service needn't reply a message
    two_way: bool,
    // 1bit Indicate whether the message is a event message
    event: bool,
    // 5bit Indicate the serialization type of the message
    serialization_type: SerializationType,
    // 1byte Indicate the status of the message
    status: MessageStatus,

    // 8byte Indicate the id of the message
    id: u64,

    // 4byte Indicate the length of the message
    data_length: usize,
}

impl Default for DubboHeader {
    fn default() -> Self {
        DubboHeader {
            msg_type: MessageType::Request,
            two_way: true,
            event: false,
            serialization_type: SerializationType::Hessian2,
            status: MessageStatus::Ok,
            id: 0,
            data_length: 0,
        }
    }
}

#[derive(Debug)]
pub enum DubboBody {
    Request(RequestInfo),
    Response(ResponseInfo),
}

#[derive(Debug)]
pub struct DubboMessage {
    pub header: DubboHeader,
    pub body: Option<DubboBody>,
}

#[derive(Debug)]
pub struct RequestInfo {
    version: String,
    service_name: String,
    method_name: String,
    method_paramter_type: Vec<String>,
    method_arguments: Vec<Box<dyn Any>>,
    attachments: HashMap<String, Box<dyn Any>>,
}

#[derive(Debug)]
pub struct ResponseInfo {
    // todo: the object should implement the trait like Into<Common Object>, From<Common Object>
    // the common Object may be protobuf struct or like serde::Serialize and serde::Deserialize
    result: Box<dyn Any>,
    exception: Option<Box<dyn std::error::Error>>,
    attachments: HashMap<String, Box<dyn Any>>,
}

pub struct DubboCodec {
    state: DecodeState,
}




#[derive(Debug, Clone, Copy)]
enum DecodeState {
    Head,
    Data(DubboHeader),
}

impl DubboMessage {
    pub fn new(header: DubboHeader) -> Self {
        DubboMessage { header, body: None }
    }

}

//
impl DubboCodec {
    pub fn new()->Self {
        DubboCodec { state: DecodeState::Head  }
    }

    fn decode_header(&mut self, src: &mut BytesMut) -> Result<Option<DubboHeader>, CodecError> {
        if src.len() < DEFAULT_HEAD_SIZE {
            return Ok(None)
        }

        let mut protocol_header = src.split_to(DEFAULT_HEAD_SIZE);


        if protocol_header.get_u16() != 0xdabb {
            return Err(CodecError::InvalidMagicCode);
        }

        let mut header = DubboHeader::default();

        // BitField |   0   | 1     | 2    | 3 -------------  7/
        //          |msg_type|two_way|event|serialization_type|
        //         | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
        let meta_value = protocol_header.get_u8();
        header.msg_type = match meta_value & 0x80 {
            0x80 => MessageType::Request,
            _ => MessageType::Response,
        };

        header.two_way = match meta_value & 0x40 {
            0x40 => true,
            _ => false,
        };

        header.event = match meta_value & 0x20 {
            0x20 => true,
            _ => false,
        };

        header.serialization_type = match meta_value & 0x1f{
            2 => SerializationType::Hessian2,
            4 => SerializationType::MsgPack,
            21 => SerializationType::Protobuf,
            _ => return Err(CodecError::InvalidSerializationType(meta_value & 0x1f)),
        };

        header.status = match protocol_header.get_u8() {
            20 => MessageStatus::Ok,
            30 => MessageStatus::ClientTimeout,
            31 => MessageStatus::ServerTimeout,
            40 => MessageStatus::BadRequest,
            50 => MessageStatus::BadResponse,
            60 => MessageStatus::ServiceNotFound,
            70 => MessageStatus::ServiceError,
            80 => MessageStatus::ServerError,
            90 => MessageStatus::ClientError,
            100 => MessageStatus::ServerThreadPoolExhastedError,
            // Maybe should return error if it's a response, but for now, just return unknow
            v => MessageStatus::Unknow(v),
        };

        header.id = protocol_header.get_u64();

        header.data_length = protocol_header.get_u32() as usize;
        // Because java use int to represent the length of the message, so the max length of the message is 2^31 - 1
        if header.data_length > DEFAULT_MAX_MESSAGE_SIZE {
            return Err(CodecError::InvalidDataLength(header.data_length))
        }

        Ok(Some(header))
    }

    fn decode_data(&mut self, header: DubboHeader, src: &mut BytesMut) -> Result<Option<DubboMessage>, CodecError> {
        if src.len() < header.data_length as usize {
            return Ok(None)
        }

        let data = src.split_to(header.data_length);

        // ser/deser impl decode the data
        Ok(Some(DubboMessage::new(header)))
    }


    fn encode_body(&mut self, item: DubboMessage) -> Result<(), CodecError> {
        Ok(())
    }

    fn encode_header(&mut self, item: DubboMessage) -> Result<(), CodecError> {
        Ok(())
    }
}

impl Decoder for DubboCodec {

    type Item = DubboMessage;
    type Error = CodecError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let header = match self.state {
            DecodeState::Head => {
                let header = self.decode_header(src)?;
                match header {
                    Some(header) => {
                        self.state = DecodeState::Data(header);
                        header
                    }
                    None => return Ok(None),
                }
            }
            DecodeState::Data(header) => header,
        };

        match self.decode_data(header, src)? {
            Some(data) => {
                // Update the decode state
                self.state = DecodeState::Head;

                Ok(Some(data))
            }
            None => Ok(None),
        }

    }

}

impl Encoder<DubboMessage> for DubboCodec {
    type Error = CodecError;



    fn encode(&mut self, item: DubboMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        // ser/deser impl encode the data
        Ok(())
    }

}



#[cfg(test)]
mod tests {
    use crate::codec::{DubboCodec, MessageType};
    #[test]
    fn test_dubbo_request_header_decode() {
        use bytes::{BufMut, BytesMut};

        let mut buf = vec![];
        buf.put_u16(0xdabb);
        // msg_type: 1, two_way: 0, event: 0, serialization_type: 2
        buf.put_u8(0b1000_0010);
        // status
        buf.put_u8(0);
        buf.put_u64(100);
        buf.put_u32(10);

        let mut bytes = BytesMut::from(buf.as_slice());

        let mut codec = DubboCodec::new();
        let header = codec.decode_header(&mut bytes).unwrap().unwrap();
        println!("{:?}", header);
        assert!(header.msg_type == MessageType::Request);
        assert!(header.two_way == false);
        assert!(header.event == false);
        assert!(header.serialization_type == crate::codec::SerializationType::Hessian2);
        assert!(header.id == 100);
        assert!(header.data_length == 10);
    }

    #[test]
    fn test_dubbo_request_header_with_invalid_datalength() {
        use bytes::{BufMut, BytesMut};
        use crate::error::CodecError;

        let mut buf = vec![];
        buf.put_u16(0xdabb);
        buf.put_u8(0b1000_0010);
        // status
        buf.put_u8(10 as u8);
        buf.put_u64(100);
        buf.put_u32(0x7fffffff);

        let mut bytes = BytesMut::from(buf.as_slice());

        let mut codec = DubboCodec::new();
        let err = codec.decode_header(&mut bytes);
        assert_eq!(err.is_err(), true);
    }

}