use bbqueue::{Producer, Consumer, GrantW, GrantR};
use core::ops::{Deref, DerefMut};
use crate::cobs::{CobsDecoder, DecoderStatus};

pub struct CobsRxGrantR {
    data_grant: GrantR,
    info_grant: GrantR,
    packet_size: usize,
}

impl Deref for CobsRxGrantR {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.data_grant[..self.packet_size]
    }
}

pub struct CobsRxGrantW {
    data_grant: GrantW,
    info_grant: GrantW,
}

impl Deref for CobsRxGrantW {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.data_grant
    }
}

impl DerefMut for CobsRxGrantW {
    fn deref_mut(&mut self) -> &mut [u8] {
        self.data_grant.buf()
    }
}

pub struct CobsRxProducer {
    data_producer: Producer,
    info_producer: Producer,
    decoder: CobsDecoder,
    current_packet_size: usize,
}

impl CobsRxProducer {
    pub fn new(data_producer: Producer, info_producer: Producer) -> Self {
        Self {
            data_producer,
            info_producer,
            decoder: CobsDecoder::new(),
            current_packet_size: 0,
        }
    }
    
    pub fn grant(&mut self, size: usize) -> Option<CobsRxGrantW> {
        if let Ok(data_grant) = self.data_producer.grant(size) {
            if let Ok(info_grant) = self.info_producer.grant(size * 2) {
                return Some(CobsRxGrantW {
                    data_grant,
                    info_grant
                })
            }

            self.data_producer.commit(0, data_grant)
        }
        None
    }

    pub fn commit(&mut self, size: usize, mut grant: CobsRxGrantW) {
        if size == 0 {
            self.data_producer.commit(0, grant.data_grant);
            self.info_producer.commit(0, grant.info_grant);
        } else {
            let mut info_buffer = &mut grant.info_grant[..];
            let mut info_buffer_size = 0;

            let mut buffer = &mut grant.data_grant[..size];
            let mut data_buffer_size = 0;
            while !buffer.is_empty() {
                let (raw_size, data_size, status) = self.decoder.decode(buffer);
                data_buffer_size += data_size;
                self.current_packet_size += data_size;

                match status {
                    DecoderStatus::InProgress => {
                        break;
                    },
                    DecoderStatus::Finished | DecoderStatus::Error => {
                        let size_bytes = (self.current_packet_size as u16).to_ne_bytes();
                        self.current_packet_size = 0;

                        info_buffer[..2].copy_from_slice(&size_bytes);
                        info_buffer = &mut info_buffer[2..];
                        info_buffer_size += 2;

                        // Move buffer tail
                        let tail_size = buffer.len() - raw_size;
                        for i in 0..tail_size {
                            buffer[data_size + i] = buffer[raw_size + i];
                        }

                        buffer = &mut buffer[data_size..data_size + tail_size];
                    },
                }

                if status == DecoderStatus::Finished {

                } else {
                    break;
                }
            }

            self.data_producer.commit(data_buffer_size, grant.data_grant);
            self.info_producer.commit(info_buffer_size, grant.info_grant);
        }
    }
}

pub struct CobsRxConsumer {
    data_consumer: Consumer,
    info_consumer: Consumer,
}

impl CobsRxConsumer {
    pub fn new(data_consumer: Consumer, info_consumer: Consumer) -> Self {
        Self {
            data_consumer,
            info_consumer
        }
    }

    pub fn read(&mut self) -> Option<CobsRxGrantR> {
        if let Ok(info_grant) = self.info_consumer.read() {
            if info_grant.len() >= 2 {
                let packet_size = u16::from_ne_bytes([info_grant[0], info_grant[1]]) as usize;
                if let Ok(data_grant) = self.data_consumer.read() {
                    if data_grant.len() >= packet_size {
                        return Some(CobsRxGrantR {
                            data_grant,
                            info_grant,
                            packet_size,
                        })
                    }

                    self.data_consumer.release(0, data_grant);
                }
            }

            self.info_consumer.release(0, info_grant);
        }
        None
    }

    /// Release packet grant and consume all the packet data
    pub fn release_consume(&mut self, grant: CobsRxGrantR) {
        self.data_consumer.release(grant.packet_size, grant.data_grant);
        self.info_consumer.release(2, grant.info_grant);
    }

    /// Release packet grant without consuming the packet
    pub fn release_unread(&mut self, grant: CobsRxGrantR) {
        self.data_consumer.release(0, grant.data_grant);
        self.info_consumer.release(0, grant.info_grant);
    }
}
