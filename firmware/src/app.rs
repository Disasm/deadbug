use stm32_usbd::UsbBusType;
use usbd_serial::USB_CLASS_CDC;
use usb_device::prelude::*;
use usb_device::bus::UsbBusAllocator;
use bbqueue::BBQueue;
use crate::cobs_rx::{CobsRxProducer, CobsRxConsumer};
use crate::cobs_tx::CobsTxProducer;
use crate::smart_serial::SmartSerial;

pub struct AppDevices {
    pub bus: UsbBusAllocator<UsbBusType>,
}

static mut RX_DATA_BUFFER: [u8; 1024] = [0; 1024];
static mut RX_INFO_BUFFER: [u8; 256] = [0; 256];
static mut TX_DATA_BUFFER: [u8; 1024] = [0; 1024];

pub fn app_run(devices: AppDevices) -> ! {
    let usb_bus = devices.bus;

    // Build queues
    let rx_data_queue = unsafe { BBQueue::unpinned_new(&mut RX_DATA_BUFFER) };
    let rx_info_queue = unsafe { BBQueue::unpinned_new(&mut RX_INFO_BUFFER) };
    let tx_data_queue = unsafe { BBQueue::unpinned_new(&mut TX_DATA_BUFFER) };
    let (rx_data_producer, rx_data_consumer) = rx_data_queue.split();
    let (rx_info_producer, rx_info_consumer) = rx_info_queue.split();
    let (tx_data_producer, tx_data_consumer) = tx_data_queue.split();
    let serial_producer = CobsRxProducer::new(rx_data_producer, rx_info_producer);
    let serial_consumer = tx_data_consumer;
    let packet_producer = CobsTxProducer::new(tx_data_producer);
    let packet_consumer = CobsRxConsumer::new(rx_data_consumer, rx_info_consumer);

    let mut proc = LoopbackProcessor {
        producer: packet_producer,
        consumer: packet_consumer,
    };

    let mut serial = SmartSerial::new(&usb_bus, serial_producer, serial_consumer);

    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x16c0, 0x27dd))
        .manufacturer("Fake company")
        .product("Serial port")
        .serial_number("TEST")
        .device_class(USB_CLASS_CDC)
        .build();

    loop {
        if usb_dev.poll(&mut [&mut serial]) {
        }
        serial.process();
        proc.process();
    }
}


struct LoopbackProcessor {
    producer: CobsTxProducer,
    consumer: CobsRxConsumer,
}

impl LoopbackProcessor {
    pub fn process(&mut self) {
        if let Some(read_grant) = self.consumer.read() {
            if read_grant.len() > 256 {
                // Too large packet
                self.consumer.release_consume(read_grant);
            } else {
                if let Some(mut write_grant) = self.producer.grant(read_grant.len()) {
                    write_grant.copy_from_slice(&read_grant);
                    self.consumer.release_consume(read_grant);
                    self.producer.commit(write_grant);
                } else {
                    self.consumer.release_unread(read_grant);
                }
            }
        }
    }
}
