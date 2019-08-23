#![no_std]
#![no_main]

//extern crate panic_semihosting;

use cortex_m::asm::delay;
use cortex_m_rt::entry;
use stm32_usbd::UsbBus;
use stm32f3xx_hal::{prelude::*, stm32, hal::digital::v2::OutputPin};
use log::{info, error};
use core::panic::PanicInfo;

mod app;
mod cobs;
mod cobs_tx;
mod command_processor;
mod dumb_serial;
mod packet_processor;
#[allow(unused)]
mod smart_serial;
mod targets;

use targets::f3_disco::BoardGpioPinSet;

fn configure_usb_clock() {
    let rcc = unsafe { &*stm32::RCC::ptr() };
    rcc.cfgr.modify(|_, w| w.usbpre().set_bit());
}

#[panic_handler]
fn panic_handler(panic_info: &PanicInfo) -> ! {
    error!("panic! {}", panic_info);
    loop {
        log::logger().flush();
    }
}

#[entry]
fn main() -> ! {
    let dp = stm32::Peripherals::take().unwrap();

    let mut flash = dp.FLASH.constrain();
    let mut rcc = dp.RCC.constrain();

    let clocks = rcc
        .cfgr
        .sysclk(48.mhz())
        .pclk1(24.mhz())
        .pclk2(24.mhz())
        .freeze(&mut flash.acr);

    let gpioc = dp.GPIOC.split(&mut rcc.ahb);
    stm32_log::configure(dp.USART1, gpioc.pc4, gpioc.pc5, 115_200.bps(), clocks);
    log::set_max_level(log::LevelFilter::Trace);
    //log::set_max_level(log::LevelFilter::Error);

    info!("========================================");
    error!("clocks: sysclk={}, hclk={}", clocks.sysclk().0, clocks.hclk().0);

    let mut gpioa = dp.GPIOA.split(&mut rcc.ahb);

    // F3 Discovery board has a pull-up resistor on the D+ line.
    // Pull the D+ pin down to send a RESET condition to the USB bus.
    let mut usb_dp = gpioa.pa12.into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper);
    let _ = usb_dp.set_low();
    delay(clocks.sysclk().0 / 100);

    let usb_dm = gpioa.pa11.into_af14(&mut gpioa.moder, &mut gpioa.afrh);
    let usb_dp = usb_dp.into_af14(&mut gpioa.moder, &mut gpioa.afrh);

    configure_usb_clock();

    let usb_bus = UsbBus::new(dp.USB, (usb_dm, usb_dp));

    let devices = app::AppDevices {
        bus: usb_bus,
        pins: BoardGpioPinSet::new(),
    };
    app::app_run(devices)
}
