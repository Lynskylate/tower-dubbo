use std::error;
use std::fmt;
use hessian_rs::Error as HessianError;

#[derive(Debug)]
pub enum CodecError {
    InvalidMagicCode,
    InvalidBody,
    InvalidSerializationType(u8),
    InvalidDataLength(usize),
    IoError(std::io::Error),
    SerializeError(String),
}

impl From<std::io::Error> for CodecError {
    fn from(err: std::io::Error) -> Self {
        CodecError::IoError(err)
    }
}

impl From<HessianError> for CodecError {
    fn from(err: HessianError) -> Self {
        CodecError::SerializeError(err.to_string())
    }
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodecError::InvalidMagicCode => write!(f, "Invalid Magic Codec"),
            CodecError::InvalidBody => write!(f, "Invalid Body"),
            CodecError::InvalidDataLength(size) => write!(f, "Invalid DataSize {}", size),
            CodecError::InvalidSerializationType(v) => {
                write!(f, "Invalid Serialization Type {}", v)
            }
            CodecError::IoError(err) => write!(f, "IoError {}", err),
            CodecError::SerializeError(err) => write!(f, "SerializeError {}", err),
        }
    }
}

impl error::Error for CodecError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            CodecError::IoError(err) => Some(err),
            _ => None,
        }
    }
}
