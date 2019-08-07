use usbd_serial::{SerialPort, DefaultBufferStore};
use crate::cobs_rx::CobsRxProducer;
use crate::cobs_tx::CobsTxConsumer;
use usb_device::class_prelude::*;
use usb_device::Result;

pub struct SmartSerial<'a, B: UsbBus> {
    inner: SerialPort<'a, B, DefaultBufferStore, DefaultBufferStore>,
    producer: CobsRxProducer,
    consumer: CobsTxConsumer,
}

impl<'a, B: UsbBus> SmartSerial<'a, B>
{
    /// Creates a new USB serial port with the provided UsbBus and 128 byte read/write buffers.
    pub fn new(alloc: &'a UsbBusAllocator<B>, producer: CobsRxProducer, consumer: CobsTxConsumer) -> Self
    {
        Self {
            inner: SerialPort::new(alloc),
            producer,
            consumer,
        }
    }

    pub fn process(&mut self) {
        if let Some(mut grant) = self.producer.grant(64) {
            if let Ok(size) = self.inner.read(&mut grant) {
                self.producer.commit(size, grant);
            } else {
                self.producer.commit(0, grant);
            }
        }

        if let Ok(grant) = self.consumer.read() {
            if let Ok(size) = self.inner.write(&grant) {
                self.consumer.release(size, grant);
            } else {
                self.consumer.release(0, grant);
            }
        }
    }
}

impl<'a, B: UsbBus> UsbClass<B> for SmartSerial<'a, B> {
    fn get_configuration_descriptors(&self, writer: &mut DescriptorWriter) -> Result<()> {
        self.inner.get_configuration_descriptors(writer)
    }

    fn reset(&mut self) {
        // TODO: discard data in queues
        self.inner.reset();
    }

    fn control_out(&mut self, xfer: ControlOut<B>) { self.inner.control_out(xfer); }

    fn control_in(&mut self, xfer: ControlIn<B>) { self.inner.control_in(xfer); }

    fn endpoint_in_complete(&mut self, addr: EndpointAddress) {
        self.inner.endpoint_in_complete(addr)
    }
}
