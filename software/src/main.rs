use serialport::{available_ports, SerialPortType, SerialPort};
use rand::distributions::Alphanumeric;
use rand::Rng;
use std::time::Duration;
use std::{io, thread};
use deadbug_common::hal::{HalError, HalResult, HalErrorKind};
use deadbug_common::protocol::channels::{CommandChannel, PacketChannel, SharedCommandChannel, SharedEndpointChannel};
use deadbug_common::protocol::gpio::{GpioCommand, GpioPinInformation};
use deadbug_common::hal::gpio::GpioPinMode;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::sync::Arc;
use embedded_hal::digital;
use embedded_hal::digital::v2::OutputPin;


fn find_device_port() -> Option<String> {
    if let Ok(list) = available_ports() {
        for info in list {
            if let SerialPortType::UsbPort(usb_info) = info.port_type {
                if usb_info.vid == 0x16c0 && usb_info.pid == 0x27dd {
                    return Some(info.port_name);
                }
            }
        }
    }
    None
}

struct CobsSerialPort {
    inner: Box<dyn SerialPort>,
    buffer: Vec<u8>,
}

impl CobsSerialPort {
    pub fn new(serial_port: Box<dyn SerialPort>) -> Self {
        Self {
            inner: serial_port,
            buffer: vec![]
        }
    }

    fn fill_buf(&mut self) -> io::Result<()> {
        if self.buffer.contains(&0) {
            return Ok(());
        }
        let mut buf = [0; 128];
        loop {
            let size = self.inner.read(&mut buf)?;
            println!("read chink : {:?}", &buf[..size]);
            self.buffer.extend_from_slice(&buf[..size]);
            if buf.contains(&0) {
                return Ok(());
            }
        }
    }
}

impl PacketChannel for CobsSerialPort {
    fn read_packet(&mut self) -> io::Result<Vec<u8>> {
        self.fill_buf()?;
        println!("filled buffer {:?}", self.buffer);

        while !self.buffer.is_empty() && self.buffer[0] == 0 {
            self.buffer.remove(0);
        }

        let pos = self.buffer.iter().enumerate().find(|(_, b)| **b == 0).map(|(i, _)| i).unwrap();
        let (packet_buf, tail) = self.buffer.split_at(pos);
        let tail = tail[1..].to_vec();
        println!("decoding {:?}", packet_buf);
        let data = cobs::decode_vec(&packet_buf).unwrap();
        self.buffer = tail;
        Ok(data)
    }

    fn write_packet(&mut self, data: &[u8]) -> io::Result<()> {
        let mut data = cobs::encode_vec(data);
        data.push(0);
        self.inner.write_all(&data)?;
        self.inner.flush()
    }
}

struct BridgeDevice {
    channel: SharedCommandChannel
}

impl BridgeDevice {
    pub fn new(channel: Box<dyn CommandChannel>) -> Self {
        Self {
            channel: SharedCommandChannel::new(channel)
        }
    }

    pub fn gpio(&self) -> HalResult<GpioPeripheral> {
        let ep_channel = SharedEndpointChannel::new(self.channel.clone(), 1);
        GpioPeripheral::probe(ep_channel)
    }
}

struct GpioBridge {
    channel: SharedEndpointChannel,
}

impl GpioBridge {
    fn enumerate(&self) -> HalResult<Vec<GpioPinInformation>> {
        let command = GpioCommand::EnumeratePins;
        let mut buf = [0; 16];
        let size = ssmarshal::serialize(&mut buf, &command).unwrap();
        let response = self.channel.command(&buf[..size])?;
        if response.len() < 1 {
            return Err(HalErrorKind::ProtocolError.into());
        }
        let n = response[0] as usize;
        let mut result = Vec::new();
        let mut offset = 1;
        for _ in 0..n {
            let (item, size) = ssmarshal::deserialize(&response[offset..]).map_err(|_| HalError::from(HalErrorKind::ProtocolError))?;
            result.push(item);
            offset += size;
        }
        if offset != response.len() {
            return Err(HalErrorKind::ProtocolError.into());
        }
        Ok(result)
    }

    fn simple_command<'a, C: Serialize, R: DeserializeOwned>(&self, command: C) -> HalResult<R> {
        let mut buf = [0; 16];
        let size = ssmarshal::serialize(&mut buf, &command).unwrap();
        let response = self.channel.command(&buf[..size])?;
        let response = ssmarshal::deserialize(&response).unwrap().0;
        Ok(response)
    }

    fn get_pin_mode(&self, index: u8) -> HalResult<GpioPinMode> {
        self.simple_command(GpioCommand::GetPinMode(index))
    }

    fn set_pin_mode(&self, index: u8, mode: GpioPinMode) -> HalResult<()> {
        self.simple_command(GpioCommand::SetPinMode(index, mode))
    }

    fn get_pin_value(&self, index: u8) -> HalResult<bool> {
        self.simple_command(GpioCommand::GetPinValue(index))
    }

    fn set_pin_value(&self, index: u8, value: bool) -> HalResult<()> {
        self.simple_command(GpioCommand::SetPinValue(index, value))
    }
}

struct GpioPeripheral {
    pins: HashMap<u8, GpioPin>,
}

impl GpioPeripheral {
    fn probe(channel: SharedEndpointChannel) -> HalResult<Self> {
        let bridge = Arc::new(GpioBridge {
            channel,
        });
        let pin_info = bridge.enumerate()?;

        let pins: HashMap<_, _> = pin_info.iter().enumerate().map(|(i, info)| {
            let pin = GpioPin {
                bridge: bridge.clone(),
                index: i as u8
            };
            (info.index_minor, pin)
        }).collect();

        Ok(Self {
            pins,
        })
    }

    pub fn pin(&mut self, index_minor: u8) -> HalResult<GpioPin> {
        self.pins.remove(&index_minor).ok_or_else(|| HalError::from(HalErrorKind::InvalidParameter))
    }

    pub fn all_pins(&mut self) -> Vec<GpioPin> {
        let mut pins: Vec<_> = self.pins.drain().map(|(_, pin)| pin).collect();
        pins.sort_unstable_by(|a, b| a.index.cmp(&b.index));
        pins
    }
}

struct GpioPin {
    bridge: Arc<GpioBridge>,
    index: u8,
}

impl GpioPin {
    pub fn into_output(&self) -> HalResult<()> {
        self.bridge.set_pin_mode(self.index, GpioPinMode::PushPullOutput)
    }
}

impl digital::v2::OutputPin for GpioPin {
    type Error = HalError;

    fn set_low(&mut self) -> Result<(), Self::Error> {
        self.bridge.set_pin_value(self.index, false)
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        self.bridge.set_pin_value(self.index, true)
    }
}

fn led_test(port: Box<dyn SerialPort>) -> HalResult<()> {
    let port = CobsSerialPort::new(port);

    let bridge = BridgeDevice::new(Box::new(port));
    let mut gpio = bridge.gpio()?;

    let mut pins = gpio.all_pins();
    for pin in &pins {
        pin.into_output()?;
    }

    let ten_millis = Duration::from_millis(100);
    loop {
        for i in 0..pins.len() {
            let next = (i + 2) % pins.len();
            pins[next].set_high()?;
            pins[i].set_low()?;
            thread::sleep(ten_millis);
        }
    }
}

#[allow(unused)]
fn rng_test(mut port: Box<dyn SerialPort>) {
    let mut rng = rand::thread_rng();
    for packet_size in 2..2048 {
        //let packet_size = 64;
        let data: String = (0..packet_size).map(|_| rng.sample(Alphanumeric)).collect();
        let mut packet_data = cobs::encode_vec(&data.as_bytes());
        packet_data.push(0);

        println!("writing {}-byte packet ({} encoded)", packet_size, packet_data.len());
        println!("{:?}", packet_data);
        let mut total = 0;
        while total < packet_data.len() {
            let count = std::cmp::min(packet_data.len() - total, 1024);
            let count = port.write(&packet_data[total..total+count]).unwrap();

            total += count as usize;

            println!("write {}", total);
        }

        println!("reading back...");

        let rx_data = packet_data.clone();
        let mut total = 0;
        let mut buf = [0u8; 1024];

        while total < rx_data.len() {
            let count = port.read(&mut buf).unwrap();

            println!("read: {} / {}", total + count, rx_data.len());

            let received = &buf[..count as usize];
            let expected = &rx_data[total..total+count as usize];

            if received != expected {
                panic!("mismatch at {} ({:?} != {:?})", total, received, expected);
            }

            total += count as usize;
        }
    }
}

fn main() {
    let port_path = if let Some(path) = find_device_port() {
        path
    } else {
        println!("Can't find device!");
        return;
    };

    let mut port = serialport::open(&port_path).unwrap();
    port.set_timeout(Duration::from_secs(1)).ok();

    // Discard any buffered leftovers
    println!("discarding...");
    loop {
        match port.read(&mut [0u8; 1024]) {
            Ok(0) => break,
            Err(ref err) if err.kind() == std::io::ErrorKind::TimedOut => break,
            Err(err) => panic!(err),
            _ => continue,
        }
    }

    println!("writing zeros...");
    port.write(&[0u8; 4]).unwrap();

    println!("running test...");
    //rng_test(port);
    led_test(port).unwrap();
}
