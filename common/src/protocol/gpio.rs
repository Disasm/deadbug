use serde::{Serialize, Deserialize};
use crate::hal::gpio::GpioPinMode;

#[derive(Debug, Serialize, Deserialize)]
pub enum GpioCommand {
    EnumeratePins,
    GetPinMode(u8),
    SetPinMode(u8, GpioPinMode),
    SetPinValue(u8, bool),
    GetPinValue(u8),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum GpioResponse {
    EnumeratePins(u8),
    GetPinMode(GpioPinMode),
    SetPinMode,
    SetPinValue,
    GetPinValue(bool),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GpioPinInformation {
    pub index_major: u8,
    pub index_minor: u8,
}
