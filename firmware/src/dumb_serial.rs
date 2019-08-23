use usbd_serial::{CdcAcmClass, LineCoding};
use usb_device::class_prelude::*;
use usb_device::Result;
use bbqueue::{Producer, Consumer};

pub struct QueuedSerial<'a, B: UsbBus> {
    inner: CdcAcmClass<'a, B>,
    producer: Producer,
    consumer: Consumer,
    write_state: WriteState,
}

/// If this many full size packets have been sent in a row, a short packet will be sent so that the
/// host sees the data in a timely manner.
const SHORT_PACKET_INTERVAL: usize = 10;

/// Keeps track of the type of the last written packet.
enum WriteState {
    /// No packets in-flight
    Idle,

    /// Short packet currently in-flight
    Short,

    /// Full packet current in-flight. A full packet must be followed by a short packet for the host
    /// OS to see the transaction. The data is the number of subsequent full packets sent so far. A
    /// short packet is forced every SHORT_PACKET_INTERVAL packets so that the OS sees data in a
    /// timely manner.
    Full(usize),
}

impl<'a, B: UsbBus> QueuedSerial<'a, B>
{
    /// Creates a new USB serial port with the provided UsbBus and buffer queues.
    pub fn new(alloc: &'a UsbBusAllocator<B>, producer: Producer, consumer: Consumer) -> Self
    {
        Self {
            inner: CdcAcmClass::new(alloc, 64),
            producer,
            consumer,
            write_state: WriteState::Idle
        }
    }

    /// Gets the current line coding.
    pub fn line_coding(&self) -> &LineCoding { self.inner.line_coding() }

    /// Gets the DTR (data terminal ready) state
    pub fn dtr(&self) -> bool { self.inner.dtr() }

    /// Gets the RTS (ready to send) state
    pub fn rts(&self) -> bool { self.inner.rts() }

    /// Returns Ok(size) if packet (even empty) was sent
    /// Returns Err(UsbError::WouldBlock) if there is no data in queue
    fn flush_write(&mut self) -> Result<usize> {
        let max_packet_size = self.inner.max_packet_size() as usize;

        let full_count = match &self.write_state {
            WriteState::Full(c) => *c,
            _ => 0,
        };

        let grant = match self.consumer.read() {
            Ok(grant) => grant,
            Err(bbqueue::Error::InsufficientSize) => {
                if full_count >= SHORT_PACKET_INTERVAL {
                    // Write ZLP
                    self.inner.write_packet(&[])?;
                    self.write_state = WriteState::Short;
                    return Ok(0);
                } else {
                    self.write_state = WriteState::Idle;
                    return Err(UsbError::WouldBlock);
                }
            },
            Err(_) => return Err(UsbError::WouldBlock),
        };

        let max_write_size = if full_count >= SHORT_PACKET_INTERVAL {
            max_packet_size - 1
        } else {
            max_packet_size
        } as usize;

        let write_size = core::cmp::min(max_write_size, grant.len());

        let r = self.inner.write_packet(&grant[..write_size]);
        let actual_write_size = *r.as_ref().unwrap_or(&0);
        self.consumer.release(actual_write_size, grant);

        if r.is_ok() {
            self.write_state = if actual_write_size == max_packet_size {
                WriteState::Full(full_count + 1)
            } else {
                WriteState::Short
            };
        }
        r
    }

    pub fn process(&mut self) -> Result<()> {
        let max_packet_size = self.inner.max_packet_size() as usize;

        while let Ok(mut grant) = self.producer.grant(max_packet_size) {
            let r = self.inner.read_packet(&mut grant);
            let read_size = *r.as_ref().unwrap_or(&0);
            self.producer.commit(read_size, grant);

            match r {
                Ok(_) => continue,
                Err(UsbError::WouldBlock) => break,
                Err(e) => return Err(e),
            }
        }

        loop {
            match self.flush_write() {
                Ok(_) => continue,
                Err(UsbError::WouldBlock) => break,
                Err(e) => return Err(e),
            }
        }

        Ok(())
    }
}

impl<'a, B: UsbBus> UsbClass<B> for QueuedSerial<'a, B> {
    fn get_configuration_descriptors(&self, writer: &mut DescriptorWriter) -> Result<()> {
        self.inner.get_configuration_descriptors(writer)
    }

    fn reset(&mut self) {
        self.inner.reset();
        // TODO: discard data in queues
        self.write_state = WriteState::Idle;
    }

    fn control_out(&mut self, xfer: ControlOut<B>) { self.inner.control_out(xfer); }

    fn control_in(&mut self, xfer: ControlIn<B>) { self.inner.control_in(xfer); }

    fn endpoint_in_complete(&mut self, addr: EndpointAddress) {
        self.inner.endpoint_in_complete(addr);
        self.flush_write().ok();
    }
}
