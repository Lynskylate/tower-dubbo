use std::fmt;
use std::error;

#[derive(Debug)]
pub enum CodecError {
    InvalidMagicCode,
    InvalidSerializationType(u8),
    InvalidDataLength(usize),
    IoError(std::io::Error),
}

impl From<std::io::Error> for CodecError {
    fn from(err: std::io::Error) -> Self {
        CodecError::IoError(err)
    }
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodecError::InvalidMagicCode => write!(f, "Invalid Magic Codec"),
            CodecError::InvalidDataLength(size) => write!(f, "Invalid DataSize {}", size),
            CodecError::InvalidSerializationType(v) => write!(f, "Invalid Serialization Type {}", v),
            CodecError::IoError(err) => write!(f, "IoError {}", err),
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
