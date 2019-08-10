use stm32_usbd::UsbBusType;
use usbd_serial::USB_CLASS_CDC;
use usb_device::prelude::*;
use usb_device::bus::UsbBusAllocator;
use bbqueue::BBQueue;
use log::info;
use crate::cobs_tx::CobsTxProducer;
use crate::smart_serial::SmartSerial;
use crate::packet_processor::{PacketProcessor, PacketConsumer};

pub struct AppDevices {
    pub bus: UsbBusAllocator<UsbBusType>,
}

static mut RX_DATA_BUFFER: [u8; 512] = [0; 512];
static mut RX_PACKET_BUFFER: [u8; 512] = [0; 512];
static mut TX_DATA_BUFFER: [u8; 512] = [0; 512];

pub fn app_run(devices: AppDevices) -> ! {
    let usb_bus = devices.bus;

    // Build queues
    let rx_data_queue = unsafe { BBQueue::unpinned_new(&mut RX_DATA_BUFFER) };
    let rx_packet_queue = unsafe { BBQueue::unpinned_new(&mut RX_PACKET_BUFFER) };
    let tx_data_queue = unsafe { BBQueue::unpinned_new(&mut TX_DATA_BUFFER) };
    let (rx_data_producer, rx_data_consumer) = rx_data_queue.split();
    let (rx_packet_producer, rx_packet_consumer) = rx_packet_queue.split();
    let (tx_data_producer, tx_data_consumer) = tx_data_queue.split();
    let mut packet_processor = PacketProcessor::new(rx_data_consumer, rx_packet_producer, 128);
    let packet_consumer = PacketConsumer::new(rx_packet_consumer);
    let packet_producer = CobsTxProducer::new(tx_data_producer);

    let mut proc = LoopbackProcessor {
        producer: packet_producer,
        consumer: packet_consumer,
    };

    let mut serial = SmartSerial::new(&usb_bus, rx_data_producer, tx_data_consumer);

    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x16c0, 0x27dd))
        .manufacturer("Fake company")
        .product("Serial port")
        .serial_number("TEST")
        .device_class(USB_CLASS_CDC)
        .build();

    loop {
        log::logger().flush();

        if usb_dev.poll(&mut [&mut serial]) {
        }
        serial.process();
        packet_processor.process();
        proc.process();
    }
}


struct LoopbackProcessor {
    producer: CobsTxProducer,
    consumer: PacketConsumer,
}

impl LoopbackProcessor {
    #[inline(never)]
    pub fn process(&mut self) {
        if let Some(read_grant) = self.consumer.read() {
            info!("got grant, len {}", read_grant.len());
            if read_grant.len() > 256 {
                // Too large packet
                info!("packet is too large ({})", read_grant.len());
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
