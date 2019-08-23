//! Debug interface based on the UART hooked up to ST-LINK

use core::fmt::{self, Write};
use stm32f3xx_hal::nb::block;
use cortex_m::interrupt;
use stm32f3xx_hal::{
    serial::{Serial, Tx},
    time::Bps,
    stm32::USART1,
    prelude::*
};
use stm32f3xx_hal::gpio::gpioc::{PC4, PC5};
use stm32f3xx_hal::rcc::Clocks;


static mut STDOUT: Option<SerialWrapper> = None;

struct SerialWrapper(Tx<USART1>);

impl SerialWrapper {
    fn write_bytes(&mut self, data: &[u8]) -> usize {
        let mut cnt = 0;
        for byte in data {
            if self.0.write(*byte).is_err() {
                break;
            }
            cnt += 1;
        }
        cnt
    }
}

impl Write for SerialWrapper {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.as_bytes() {
            if *byte == '\n' as u8 {
                let res = block!(self.0.write('\r' as u8));

                if res.is_err() {
                    return Err(::core::fmt::Error);
                }
            }

            let res = block!(self.0.write(*byte));

            if res.is_err() {
                return Err(::core::fmt::Error);
            }
        }
        Ok(())
    }
}


/// Configures stdout
pub fn configure<X, Y>(
    uart: USART1, tx: PC4<X>, rx: PC5<Y>,
    baudrate: Bps, clocks: Clocks
) {
    let mut moder = unsafe { core::mem::zeroed() };
    let mut afrl = unsafe { core::mem::zeroed() };
    let mut apb2 = unsafe { core::mem::zeroed() };
    let tx = tx.into_af7(&mut moder, &mut afrl);
    let rx = rx.into_af7(&mut moder, &mut afrl);
    let serial = Serial::usart1(uart, (tx, rx), baudrate, clocks, &mut apb2);
    let (tx, _) = serial.split();

    interrupt::free(|_| {
        unsafe {
            STDOUT.replace(SerialWrapper(tx));
        }
    });

    crate::log::init();
}

pub fn write_bytes(data: &[u8]) -> usize {
    interrupt::free(|_| unsafe {
        if let Some(stdout) = STDOUT.as_mut() {
            stdout.write_bytes(data)
        } else {
            0
        }
    })
}
