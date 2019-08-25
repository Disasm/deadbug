use serialport::{available_ports, SerialPortType, SerialPort};
use rand::distributions::Alphanumeric;
use rand::Rng;
use std::time::Duration;
use std::{io, thread};
use deadbug_common::hal::{HalError, HalResult};
use deadbug_common::protocol::channels::{CommandChannel, PacketChannel};
use deadbug_common::protocol::gpio::GpioCommand;
use deadbug_common::hal::gpio::GpioPinMode;
use serde::Serialize;
use serde::de::DeserializeOwned;


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

    pub fn command<'a, C: Serialize, R: DeserializeOwned>(&mut self, endpoint: u8, command: C) -> HalResult<R> {
        let mut buf = [0; 128];
        let size = ssmarshal::serialize(&mut buf, &command).unwrap();
        let response = CommandChannel::command(self, endpoint, &buf[..size])?;
        let response = ssmarshal::deserialize(&response).unwrap().0;
        Ok(response)
    }

    pub fn gpio_command(&mut self, command: GpioCommand) -> Result<Vec<u8>, HalError> {
        let mut buf = [0; 128];
        let size = ssmarshal::serialize(&mut buf, &command).unwrap();
        CommandChannel::command(self, 1, &buf[..size])
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

fn led_test(port: Box<dyn SerialPort>) {
    let mut port = CobsSerialPort::new(port);

    for i in 0..8 {
        port.gpio_command(GpioCommand::SetPinMode(i, GpioPinMode::PushPullOutput)).expect("can't set gpio mode");
        port.gpio_command(GpioCommand::SetPinValue(i, false)).expect("can't set gpio value");
    }
    let ten_millis = Duration::from_millis(100);
    loop {
        for i in 0..8 {
            let next = (i + 2) % 8;
            port.gpio_command(GpioCommand::SetPinValue(next, true)).expect("can't set gpio value");
            port.gpio_command(GpioCommand::SetPinValue(i, false)).expect("can't set gpio value");
            thread::sleep(ten_millis);
        }
    }
    port.gpio_command(GpioCommand::SetPinValue(0, true)).expect("can't set gpio value");

    let v = port.gpio_command(GpioCommand::GetPinValue(0)).expect("can't get gpio value");
    println!("gpio value: {:?}", v);
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
    led_test(port);
}
