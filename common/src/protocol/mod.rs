use serde::{Serialize, Deserialize};
use crate::hal::HalErrorKind;

#[cfg(feature = "std")]
pub mod channels;
pub mod gpio;

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandHeader {
    pub endpoint: u8,
}

pub type ResponseHeader = Result<(), HalErrorKind>;
