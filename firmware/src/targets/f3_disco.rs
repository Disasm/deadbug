use deadbug_common::hal::gpio::{GpioPin, GpioPinMode};
use deadbug_common::hal::{HalResult, HalErrorKind};
use stm32f3xx_hal::stm32;
use deadbug_common::protocol::gpio::GpioPinInformation;

pub struct BoardGpioPin {
    index: u8,
    mode: GpioPinMode,
}

impl BoardGpioPin {
    pub(crate) fn new(peripheral: u8, pin_index: u8) -> Self {
        assert!(peripheral < 6);
        assert!(pin_index < 16);
        Self {
            index: (peripheral << 4) | (pin_index & 0xf),
            mode: GpioPinMode::FloatingInput,
        }
    }

    #[inline(always)]
    fn peripheral(&self) -> u8 {
        self.index >> 4
    }

    #[inline(always)]
    fn pin_index(&self) -> u8 {
        self.index & 0xf
    }

    fn regs(&self) -> &'static stm32::gpioa::RegisterBlock {
        let ptr = match self.peripheral() {
            0 => stm32::GPIOA::ptr() as usize,
            1 => stm32::GPIOB::ptr() as usize,
            2 => stm32::GPIOC::ptr() as usize,
            3 => stm32::GPIOD::ptr() as usize,
            4 => stm32::GPIOE::ptr() as usize,
            5 => stm32::GPIOF::ptr() as usize,
            _ => unreachable!(),
        };
        unsafe { &*(ptr as *const stm32::gpioa::RegisterBlock) }
    }
}

impl GpioPin for BoardGpioPin {
    fn information(&self) -> GpioPinInformation {
        GpioPinInformation {
            index_major: self.peripheral() + b'A',
            index_minor: self.pin_index(),
        }
    }

    fn mode(&self) -> GpioPinMode {
        self.mode
    }

    fn set_mode(&mut self, mode: GpioPinMode) -> HalResult<()> {
        let regs = self.regs();
        match mode {
            GpioPinMode::FloatingInput => {
                let offset = self.pin_index() * 2;

                // input mode
                regs.moder.modify(|r, w| unsafe {
                    w.bits(r.bits() & !(0b11 << offset))
                });

                // no pull-up or pull-down
                regs.pupdr.modify(|r, w| unsafe {
                    w.bits(r.bits() & !(0b11 << offset))
                });
            },
            GpioPinMode::PushPullOutput => {
                let offset = self.pin_index() * 2;

                // general purpose output mode
                let mode = 0b01;
                regs.moder.modify(|r, w| unsafe {
                    w.bits((r.bits() & !(0b11 << offset)) | (mode << offset))
                });

                // push pull output
                regs.otyper.modify(|r, w| unsafe {
                    w.bits(r.bits() & !(0b1 << self.pin_index()))
                });
            }
            _ => return Err(HalErrorKind::InvalidGpioMode.into()),
        }
        self.mode = mode;
        Ok(())
    }

    fn set_output(&mut self, value: bool) -> HalResult<()> {
        if self.mode == GpioPinMode::PushPullOutput {
            let mask = if value {
                1 << self.pin_index()
            } else {
                1 << (16 + self.pin_index())
            };
            // NOTE(unsafe) atomic write to a stateless register
            unsafe { self.regs().bsrr.write(|w| w.bits(mask)); }

            Ok(())
        } else {
            Err(HalErrorKind::InvalidGpioMode.into())
        }
    }

    fn get_input(&self) -> HalResult<bool> {
        if self.mode == GpioPinMode::FloatingInput {
            let value = self.regs().idr.read().bits() & (1 << self.pin_index()) != 0;
            Ok(value)
        } else {
            Err(HalErrorKind::InvalidGpioMode.into())
        }
    }
}

pub struct BoardGpioPinSet {
    pins: [BoardGpioPin; 8]
}

impl BoardGpioPinSet {
    pub(crate) fn new() -> Self {
        let rcc = unsafe { &*stm32::RCC::ptr() };
        rcc.ahbenr.modify(|_, w| w.iopeen().set_bit());
        rcc.ahbrstr.modify(|_, w| w.ioperst().set_bit());
        rcc.ahbrstr.modify(|_, w| w.ioperst().clear_bit());

        let pins = [
            BoardGpioPin::new(4, 8),  // PE8, blue led
            BoardGpioPin::new(4, 9),  // PE9, red led
            BoardGpioPin::new(4, 10), // PE10, orange led
            BoardGpioPin::new(4, 11), // PE11, green led
            BoardGpioPin::new(4, 12), // PE12, blue led
            BoardGpioPin::new(4, 13), // PE13, red led
            BoardGpioPin::new(4, 14), // PE14, orange led
            BoardGpioPin::new(4, 15), // PE15, green led
        ];

        Self {
            pins,
        }
    }

    pub fn len(&self) -> usize {
        self.pins.len()
    }
}

impl<'a> IntoIterator for &'a BoardGpioPinSet {
    type Item = &'a BoardGpioPin;
    type IntoIter = core::slice::Iter<'a, BoardGpioPin>;

    fn into_iter(self) -> Self::IntoIter {
        self.pins.iter()
    }
}

impl<'a> IntoIterator for &'a mut BoardGpioPinSet {
    type Item = &'a mut BoardGpioPin;
    type IntoIter = core::slice::IterMut<'a, BoardGpioPin>;

    fn into_iter(self) -> Self::IntoIter {
        self.pins.iter_mut()
    }
}
