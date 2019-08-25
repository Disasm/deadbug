use serde::{Serialize, Deserialize};
use core::fmt;

pub mod gpio;

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub enum HalErrorKind {
    /// This command is not supported on this device
    UnsupportedCommand,

    InvalidParameter,

    ProtocolError,

    /// Invalid GPIO mode
    /// Can be used either for invalid method calls or invalid mode values passed
    InvalidGpioMode,

    Other(u8),
}

pub struct HalError {
    kind: HalErrorKind,
    message: Option<&'static str>,
}

impl HalError {
    pub fn new(kind: HalErrorKind, message: &'static str) -> Self {
        Self {
            kind,
            message: Some(message),
        }
    }

    pub fn kind(&self) -> HalErrorKind {
        self.kind
    }
}

impl fmt::Debug for HalError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(msg) = self.message {
            write!(f, "HalError({:?}, {})", self.kind, msg)
        } else {
            write!(f, "HalError({:?})", self.kind)
        }
    }
}

impl From<HalErrorKind> for HalError {
    fn from(kind: HalErrorKind) -> Self {
        HalError {
            kind,
            message: None,
        }
    }
}

impl From<ssmarshal::Error> for HalError {
    fn from(_: ssmarshal::Error) -> Self {
        HalErrorKind::InvalidParameter.into()
    }
}

pub type HalResult<T> = Result<T, HalError>;
