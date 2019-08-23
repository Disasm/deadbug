use serde::{Serialize, Deserialize};
use crate::hal::gpio::GpioPinMode;
use crate::hal::HalErrorKind;

#[derive(Debug, Serialize, Deserialize)]
pub enum GpioCommand {
    EnumeratePins,
    GetPinMode(u8),
    SetPinMode(u8, GpioPinMode),
    SetPinValue(u8, bool),
    GetPinValue(u8),
}

#[repr(u8)]
#[derive(Serialize, Deserialize)]
pub enum HalResultCode {
    NoError,
    Error(HalErrorKind),
}
