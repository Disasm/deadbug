use std::io;
use crate::hal::HalResult;
use crate::protocol::{CommandHeader, ResponseHeader};

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

        println!("sending packet: {:?}", command_buffer);
        self.write_packet(&command_buffer).unwrap();
        let response_buffer = self.read_packet().unwrap();
        println!("response packet: {:?}", response_buffer);

        let (header, header_size) = ssmarshal::deserialize::<ResponseHeader>(&response_buffer).unwrap();
        match header {
            Ok(()) => return Ok(response_buffer[header_size..].to_vec()),
            Err(error_kind) => return Err(error_kind.into()),
        }
    }
}
