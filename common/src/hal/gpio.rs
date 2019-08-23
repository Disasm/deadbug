use crate::hal::HalResult;
use serde::{Serialize, Deserialize};

#[derive(Debug, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum GpioPinMode {
    FloatingInput,
    PushPullOutput,
    Alternate(u8),
}

pub trait GpioPin {
    fn address(&self) -> (u8, u8);

    fn mode(&self) -> GpioPinMode;

    fn set_mode(&mut self, mode: GpioPinMode) -> HalResult<()>;

    fn set_output(&mut self, value: bool) -> HalResult<()>;

    fn get_input(&self) -> HalResult<bool>;
}
