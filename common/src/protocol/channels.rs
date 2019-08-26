use std::io;
use crate::hal::{HalResult, HalErrorKind, HalError};
use crate::protocol::{CommandHeader, ResponseHeader};
use std::sync::{Arc, Mutex};

pub trait PacketChannel {
    fn read_packet(&mut self) -> io::Result<Vec<u8>>;

    fn write_packet(&mut self, data: &[u8]) -> io::Result<()>;
}

pub trait CommandChannel {
    fn command(&mut self, endpoint: u8, command: &[u8]) -> HalResult<Vec<u8>>;
}

impl<T: PacketChannel> CommandChannel for T {
    fn command(&mut self, endpoint: u8, command: &[u8]) -> HalResult<Vec<u8>> {
        let header = CommandHeader {
            endpoint
        };
        let mut header_buffer = [0; 1];
        let header_size = ssmarshal::serialize(&mut header_buffer, &header).unwrap();

        let mut command_buffer = Vec::new();
        command_buffer.extend_from_slice(&header_buffer[..header_size]);
        command_buffer.extend_from_slice(command);

        self.write_packet(&command_buffer).map_err(|_| HalError::from(HalErrorKind::ProtocolError))?;
        let response_buffer = self.read_packet().map_err(|_| HalError::from(HalErrorKind::ProtocolError))?;

        let (header, header_size) = ssmarshal::deserialize::<ResponseHeader>(&response_buffer)
            .map_err(|_| HalError::from(HalErrorKind::ProtocolError))?;
        match header {
            Ok(()) => return Ok(response_buffer[header_size..].to_vec()),
            Err(error_kind) => return Err(error_kind.into()),
        }
    }
}

#[derive(Clone)]
pub struct SharedCommandChannel(Arc<Mutex<Box<dyn CommandChannel>>>);

impl SharedCommandChannel {
    pub fn new(channel: Box<dyn CommandChannel>) -> Self {
        Self(Arc::new(Mutex::new(channel)))
    }
}

impl CommandChannel for SharedCommandChannel {
    fn command(&mut self, endpoint: u8, command: &[u8]) -> HalResult<Vec<u8>> {
        let mut channel = self.0.lock().unwrap();
        channel.command(endpoint, command)
    }
}

impl<'a> CommandChannel for &'a SharedCommandChannel {
    fn command(&mut self, endpoint: u8, command: &[u8]) -> HalResult<Vec<u8>> {
        let mut channel = self.0.lock().unwrap();
        channel.command(endpoint, command)
    }
}

#[derive(Clone)]
pub struct SharedEndpointChannel {
    command_channel: SharedCommandChannel,
    endpoint: u8,
}

impl SharedEndpointChannel {
    pub fn new(command_channel: SharedCommandChannel, endpoint: u8) -> Self {
        Self {
            command_channel,
            endpoint
        }
    }

    pub fn command(&self, command: &[u8]) -> HalResult<Vec<u8>> {
        (&self.command_channel).command(self.endpoint, command)
    }
}
