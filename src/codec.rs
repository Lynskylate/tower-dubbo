// document: https://dubbo.apache.org/zh-cn/blog/2018/10/05/dubbo-%E5%8D%8F%E8%AE%AE%E8%AF%A6%E8%A7%A3/#codec%E7%9A%84%E5%AE%9A%E4%B9%89
use crate::constant::{DEFAULT_HEAD_SIZE, DEFAULT_MAX_MESSAGE_SIZE, DUBBO_MAGIC_CODE};
use crate::error::CodecError;

use std::any::Any;
use std::collections::HashMap;

use bytes::{Buf, BufMut, Bytes, BytesMut};
use derive_builder::Builder;
use hessian_rs::value::{List, Map as HessianMap, ToHessian, Value};
use tokio_util::codec::{self, Decoder, Encoder};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

    // According to "java dubbo" There are two cases of response:
    // 		1. with attachments
    // 		2. no attachments
    ResponseWithException = 0,
    ResponseValue = 1,
    ResponseNullValue = 2,
    ResponseWithExceptionWithAttachments = 3,
    ResponseValueWithAttachments = 4,
    ResponseNullValueWithAttachments = 5,

    Unknow = 0xff,
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

#[derive(Debug, Builder)]
pub struct RequestInfo {
    version: String,
    service_name: String,
    service_version: String,
    method_name: String,
    method_paramter_type: Vec<String>,
    method_arguments: Vec<Value>,
    // Unsupport struct value in attachments
    attachments: HashMap<String, String>,
}

#[derive(Debug)]
pub struct ResponseInfo {
    // todo: the object should implement the trait like Into<Common Object>, From<Common Object>
    // the common Object may be protobuf struct or like serde::Serialize and serde::Deserialize
    result: Option<Value>,
    exception: Option<Box<dyn std::error::Error + Send>>,
    attachments: HashMap<String, String>,
}

pub struct DubboCodec {
    state: DecodeState,
}

pub trait Serializer {
    fn serialize_request(&self, value: &DubboMessage) -> Result<Bytes, CodecError>;
    fn serialize_response(&self, value: &DubboMessage) -> Result<Bytes, CodecError>;
}

pub trait Deserializer {
    fn deserialize_request(&self, value: BytesMut) -> Result<RequestInfo, CodecError>;
    fn deserialize_response(&self, value: BytesMut) -> Result<ResponseInfo, CodecError>;
}

impl From<SerializationType> for Box<dyn Serializer> {
    fn from(value: SerializationType) -> Self {
        match value {
            SerializationType::Hessian2 => Box::new(Hessian2Serializer),
            _ => {
                panic!("not support serialization type");
            }
        }
    }
}

impl From<SerializationType> for Box<dyn Deserializer> {
    fn from(value: SerializationType) -> Self {
        match value {
            SerializationType::Hessian2 => Box::new(Hessian2Serializer),
            _ => {
                panic!("not support serialization type");
            }
        }
    }
}

pub struct Hessian2Serializer;
impl Serializer for Hessian2Serializer {
    fn serialize_request(&self, message: &DubboMessage) -> Result<Bytes, CodecError> {
        assert!(message.header.msg_type == MessageType::Request);
        let value = match message.body {
            Some(DubboBody::Request(ref value)) => value,
            _ => {
                return Err(CodecError::SerializeError(
                    "request body is empty".to_string(),
                ));
            }
        };

        let mut vec = Vec::new();
        let mut ser = hessian_rs::ser::Serializer::new(&mut vec);

        // todo: optimize copy
        ser.serialize_value(&value.version.clone().to_hessian())?;
        ser.serialize_value(&value.service_name.clone().to_hessian())?;
        ser.serialize_value(&value.service_version.clone().to_hessian())?;
        ser.serialize_value(&value.method_name.clone().to_hessian())?;
        let parameter_type_list = format!("{};", value
            .method_paramter_type
            .clone()
            .into_iter()
            .map(|k| format!("L{}", k.replace(".", "/")))
            .collect::<Vec<_>>()
            .join(";"));
        ser.serialize_value(&parameter_type_list.to_hessian())?;
        for arg in value.method_arguments.iter() {
            ser.serialize_value(&arg)?;
        }
        ser.serialize_value(&value.attachments.clone().to_hessian())?;

        Ok(Bytes::from(vec))
    }

    fn serialize_response(&self, message: &DubboMessage) -> Result<Bytes, CodecError> {
        assert!(message.header.msg_type == MessageType::Request);
        let mut vec = Vec::new();
        let mut ser = hessian_rs::ser::Serializer::new(&mut vec);
        // return heartbeat with null
        if message.header.status == MessageStatus::Ok && message.header.event {
            ser.serialize_value(&Value::Null)?;
            return Ok(Bytes::from(vec));
        }

        let value = match message.body {
            Some(DubboBody::Response(ref value)) => value,
            _ => {
                return Err(CodecError::SerializeError(
                    "response body is empty".to_string(),
                ));
            }
        };

        if let Some(ref e) = value.exception {
            ser.serialize_value(
                &(MessageStatus::ResponseWithExceptionWithAttachments as i32).to_hessian(),
            )?;
            ser.serialize_value(&e.to_string().to_hessian())?;
            ser.serialize_value(&value.attachments.clone().to_hessian())?;
        } else {
            if value.result == None {
                ser.serialize_value(
                    &(MessageStatus::ResponseNullValueWithAttachments as i32).to_hessian(),
                )?;
                ser.serialize_value(&Value::Null)?;
            } else {
                ser.serialize_value(
                    &(MessageStatus::ResponseValueWithAttachments as i32).to_hessian(),
                )?;
                ser.serialize_value(&value.result.clone().unwrap())?;
            }

            ser.serialize_value(&value.attachments.clone().to_hessian())?;
        }

        Ok(Bytes::from(vec))
    }
}

impl Deserializer for Hessian2Serializer {
    fn deserialize_request(&self, body: BytesMut) -> Result<RequestInfo, CodecError> {
        let mut de = hessian_rs::de::Deserializer::new(&body[..]);
        let dubbo_version = match de.read_value()? {
            Value::String(dubbo_version) => dubbo_version,
            _ => {
                return Err(CodecError::SerializeError(
                    "dubbo version is not string".to_string(),
                ));
            }
        };
        let dubbo_service_name = match de.read_value()? {
            Value::String(dubbo_service_name) => dubbo_service_name,
            _ => {
                return Err(CodecError::SerializeError(
                    "dubbo service name is not string".to_string(),
                ));
            }
        };
        let service_version = match de.read_value()? {
            Value::String(service_version) => service_version,
            _ => {
                return Err(CodecError::SerializeError(
                    "dubbo service version is not string".to_string(),
                ));
            }
        };
        let method_name = match de.read_value()? {
            Value::String(method_name) => method_name,
            _ => {
                return Err(CodecError::SerializeError(
                    "dubbo method name is not string".to_string(),
                ));
            }
        };
        let parameter_type = match de.read_value()? {
            Value::String(parameter_type) => parameter_type,
            _ => {
                return Err(CodecError::SerializeError(
                    "dubbo method parameter types is not string".to_string(),
                ));
            }
        };

        let method_parameter_types: Vec<_> = parameter_type
            .split(';')
            .filter(|&x| !x.is_empty())
            .map(|x| {
                let mut s = String::from(x);
                s.remove(0);
                s.replace("/", ".")
            })
            .collect();

        let mut parameters = vec![];
        for _ in 0..method_parameter_types.len() {
            let v = de.read_value()?;
            parameters.push(v)
        }

        if let Value::Map(attachments) = de.read_value()? {
            let mut request_attachments = HashMap::with_capacity(attachments.len());
            attachments.value().into_iter().for_each(|(k, v)| {
                // ignore non-string key and value
                if k.is_str() && v.is_str() {
                    request_attachments.insert(
                        k.as_str().unwrap().to_string(),
                        v.as_str().unwrap().to_string(),
                    );
                }
            });
            return Ok(RequestInfo {
                version: dubbo_version,
                service_name: dubbo_service_name,
                service_version: service_version,
                method_name: method_name,
                method_paramter_type: method_parameter_types
                    .into_iter()
                    .map(|v| v.to_string())
                    .collect(),
                method_arguments: parameters,
                attachments: request_attachments,
            });
        } else {
            return Ok(RequestInfo {
                version: dubbo_version,
                service_name: dubbo_service_name,
                service_version: service_version,
                method_name: method_name,
                method_paramter_type: method_parameter_types
                    .into_iter()
                    .map(|v| v.to_string())
                    .collect(),
                method_arguments: parameters,
                attachments: HashMap::new(),
            });
        }
    }

    fn deserialize_response(&self, value: BytesMut) -> Result<ResponseInfo, CodecError> {
        println!("deserialize_response: {:?}", value);
        todo!()
    }
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

    pub fn with_request(header: DubboHeader, body: RequestInfo) -> Self {
        DubboMessage {
            header,
            body: Some(DubboBody::Request(body)),
        }
    }

    pub fn with_response(header: DubboHeader, body: ResponseInfo) -> Self {
        DubboMessage {
            header,
            body: Some(DubboBody::Response(body)),
        }
    }

    pub fn into_request(self) -> Option<RequestInfo> {
        match self.body {
            Some(DubboBody::Request(body)) => Some(body),
            _ => None,
        }
    }

    pub fn into_response(self) -> Option<ResponseInfo> {
        match self.body {
            Some(DubboBody::Response(body)) => Some(body),
            _ => None,
        }
    }

    pub fn id(&self) -> u64 {
        self.header.id
    }

    pub fn set_id(&mut self, id: u64) {
        self.header.id = id
    }
}

//
impl DubboCodec {
    pub fn new() -> Self {
        DubboCodec {
            state: DecodeState::Head,
        }
    }

    fn decode_header(&mut self, src: &mut BytesMut) -> Result<Option<DubboHeader>, CodecError> {
        if src.len() < DEFAULT_HEAD_SIZE {
            return Ok(None);
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

        header.serialization_type = match meta_value & 0x1f {
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
            v => MessageStatus::Unknow,
        };

        header.id = protocol_header.get_u64();

        header.data_length = protocol_header.get_u32() as usize;
        // Because java use int to represent the length of the message, so the max length of the message is 2^31 - 1
        if header.data_length > DEFAULT_MAX_MESSAGE_SIZE {
            return Err(CodecError::InvalidDataLength(header.data_length));
        }

        Ok(Some(header))
    }

    fn decode_data(
        &mut self,
        header: DubboHeader,
        src: &mut BytesMut,
    ) -> Result<Option<DubboMessage>, CodecError> {
        if src.len() < header.data_length as usize {
            return Ok(None);
        }

        let data = src.split_to(header.data_length);
        let de: Box<dyn Deserializer> = From::from(header.serialization_type);
        if header.msg_type == MessageType::Request {
            let request_info = de.deserialize_request(data)?;
            Ok(Some(DubboMessage::with_request(header, request_info)))
        } else {
            let response_info = de.deserialize_response(data)?;
            Ok(Some(DubboMessage::with_response(header, response_info)))
        }
    }

    fn encode_body(&mut self, item: &DubboMessage) -> Result<Bytes, CodecError> {
        let ser: Box<dyn Serializer> = From::from(item.header.serialization_type);
        match item.body {
            Some(DubboBody::Request(ref req)) => Ok(ser.serialize_request(item)?),
            Some(DubboBody::Response(ref resp)) => Ok(ser.serialize_response(item)?),
            None => Err(CodecError::InvalidBody),
        }
    }

    fn encode_header(
        &mut self,
        header: &DubboHeader,
        buf: &mut BytesMut,
    ) -> Result<(), CodecError> {
        buf.put_u16(DUBBO_MAGIC_CODE);
        let mut meta_value = 0;
        if header.msg_type == MessageType::Request {
            meta_value |= 0x80;
        }
        if header.two_way {
            meta_value |= 0x40;
        }
        if header.event {
            meta_value |= 0x20;
        }
        meta_value |= header.serialization_type as u8;
        buf.put_u8(meta_value);
        buf.put_u8(header.status as u8);
        buf.put_u64(header.id);
        assert!(header.data_length <= DEFAULT_MAX_MESSAGE_SIZE);
        buf.put_u32(header.data_length as u32);
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
        let payload = self.encode_body(&item)?;
        let mut header = item.header;
        header.data_length = payload.len();
        self.encode_header(&header, dst)?;
        dst.extend(payload);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::codec::{DubboCodec, MessageType};
    use bytes::{BufMut, BytesMut};
    use tokio_util::codec::{Decoder, Encoder};
    use super::*;
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

    #[test]
    fn test_codec() {
        let dubbo_request_raw_bytes = &[
            0xda, 0xbb, 0xc2, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00,
            0x01, b'G', 0x05, 0x32, 0x2e, 0x30, 0x2e, 0x32, 0x30, 0x24, b'o', b'r', b'g', 0x2e,
            b'a', b'p', b'a', b'c', b'h', b'e', 0x2e, b'd', b'u', b'b', b'b', b'o', 0x2e, b's',
            b'a', b'm', b'p', b'l', b'e', 0x2e, b'U', b's', b'e', b'r', b'P', b'r', b'o', b'v',
            b'i', b'd', b'e', b'r', 0x00, 0x07, b'G', b'e', b't', b'U', b's', b'e', b'r', 0x1e,
            b'L', b'o', b'r', b'g', 0x2f, b'a', b'p', b'a', b'c', b'h', b'e', 0x2f, b'd', b'u',
            b'b', b'b', b'o', 0x2f, b's', b'a', b'm', b'p', b'l', b'e', 0x2f, b'U', b's', b'e',
            b'r', 0x3b, b'C', 0x1c, b'o', b'r', b'g', 0x2e, b'a', b'p', b'a', b'c', b'h', b'e',
            0x2e, b'd', b'u', b'b', b'b', b'o', 0x2e, b's', b'a', b'm', b'p', b'l', b'e', 0x2e,
            b'U', b's', b'e', b'r', 0x95, 0x02, b'i', b'd', 0x04, b'n', b'a', b'm', b'e', 0x03,
            b'a', b'g', b'e', 0x04, b't', b'i', b'm', b'e', 0x03, b's', b'e', b'x', 0x60, 0x03,
            0x30, 0x30, 0x33, 0x00, 0x90, b'N', b'C', 0x1e, b'o', b'r', b'g', 0x2e, b'a', b'p',
            b'a', b'c', b'h', b'e', 0x2e, b'd', b'u', b'b', b'b', b'o', 0x2e, b's', b'a', b'm',
            b'p', b'l', b'e', 0x2e, b'G', b'e', b'n', b'd', b'e', b'r', 0x91, 0x04, b'n', b'a',
            b'm', b'e', b'a', 0x03, b'M', b'A', b'N', b'H', 0x09, b'i', b'n', b't', b'e', b'r',
            b'f', b'a', b'c', b'e', 0x30, 0x24, b'o', b'r', b'g', 0x2e, b'a', b'p', b'a', b'c',
            b'h', b'e', 0x2e, b'd', b'u', b'b', b'b', b'o', 0x2e, b's', b'a', b'm', b'p', b'l',
            b'e', 0x2e, b'U', b's', b'e', b'r', b'P', b'r', b'o', b'v', b'i', b'd', b'e', b'r',
            0x07, b't', b'i', b'm', b'e', b'o', b'u', b't', 0x01, 0x30, 0x07, b'v', b'e', b'r',
            b's', b'i', b'o', b'n', 0x00, 0x05, b'a', b's', b'y', b'n', b'c', 0x05, b'f', b'a',
            b'l', b's', b'e', 0x0b, b'e', b'n', b'v', b'i', b'r', b'o', b'n', b'm', b'e', b'n',
            b't', 0x03, b'd', b'e', b'v', 0x04, b'p', b'a', b't', b'h', 0x30, 0x24, b'o', b'r',
            b'g', 0x2e, b'a', b'p', b'a', b'c', b'h', b'e', 0x2e, b'd', b'u', b'b', b'b', b'o',
            0x2e, b's', b'a', b'm', b'p', b'l', b'e', 0x2e, b'U', b's', b'e', b'r', b'P', b'r',
            b'o', b'v', b'i', b'd', b'e', b'r', b'Z',
        ];
        let mut buf = BytesMut::new();
        buf.put_slice(dubbo_request_raw_bytes);
        let mut codec = DubboCodec::new();
        let res = codec.decode(&mut buf).unwrap().unwrap();
        println!("{:?}", res);

        let dubbo_request = res.into_request().unwrap();
        assert_eq!(
            dubbo_request.service_name,
            "org.apache.dubbo.sample.UserProvider"
        );
    }

    #[test]
    fn test_codec_request_roundtrip() {
        let mut codec = DubboCodec::new();

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

        // encode and decode
        let req = DubboMessage::with_request(header, body);
        let mut dst = BytesMut::new();
        codec.encode(req, &mut dst).unwrap();
        let codec_res =  codec.decode(&mut dst).unwrap().unwrap();
        let dubbo_request = codec_res.into_request().unwrap();

        assert_eq!(dubbo_request.service_name, "org.apache.dubbo.sample.UserProvider");
        assert_eq!(dubbo_request.method_name, "GetUser");
        assert_eq!(dubbo_request.version, "1.0.2");
        assert_eq!(dubbo_request.service_version, "");
        assert_eq!(dubbo_request.method_paramter_type, vec![String::from("org.apache.dubbo.sample.User")]);

    }
}
