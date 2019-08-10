use bbqueue::{Consumer, Producer, GrantW, GrantR};
use core::mem;
use core::ops::Deref;

#[derive(PartialEq)]
enum PacketProcessorState {
    Discarding,
    WaitingForGrant,
    Processing(GrantW, usize),
}

pub struct PacketProcessor {
    consumer: Consumer,
    producer: Producer,
    state: PacketProcessorState,
    max_data_size: usize,
}

impl PacketProcessor {
    pub fn new(consumer: Consumer, producer: Producer, max_packet_size: usize) -> Self {
        Self {
            consumer,
            producer,
            state: PacketProcessorState::Discarding,
            max_data_size: cobs::max_encoding_length(max_packet_size) + 1,
        }
    }

    pub fn process(&mut self) {
        if let Ok(grant_r) = self.consumer.read() {
            let zero_pos = grant_r.iter().enumerate().find(|&(_, v)| *v == 0).map(|(i, _)| i);
            let chunk_size = zero_pos.map(|i| i + 1).unwrap_or(grant_r.len());

            match &mut self.state {
                PacketProcessorState::Discarding => {
                    if zero_pos.is_some() {
                        self.state = PacketProcessorState::WaitingForGrant;
                    }
                    self.consumer.release(chunk_size, grant_r);
                },
                PacketProcessorState::WaitingForGrant => {
                    if let Ok(grant_w) = self.producer.grant(2 + self.max_data_size) {
                        self.state = PacketProcessorState::Processing(grant_w, 2);
                    }
                    self.consumer.release(0, grant_r);
                },
                PacketProcessorState::Processing(_, _) => {
                    let state = mem::replace(&mut self.state, PacketProcessorState::WaitingForGrant);
                    if let PacketProcessorState::Processing(mut grant_w, data_size) = state {
                        let new_data_size = data_size + chunk_size;

                        if new_data_size <= grant_w.len() {
                            // Copy raw data
                            grant_w[data_size..data_size + chunk_size].copy_from_slice(&grant_r[..chunk_size]);

                            if zero_pos.is_none() {
                                // Continue processing, update state
                                self.state = PacketProcessorState::Processing(grant_w, new_data_size);
                            } else {
                                // Zero byte found, decode
                                if let Ok(packet_size) = cobs::decode_in_place(&mut grant_w[2..new_data_size - 1]) {
                                    if packet_size > 0 {
                                        // Write the actual packet length
                                        let len_bytes = (packet_size as u16).to_ne_bytes();
                                        grant_w[..2].copy_from_slice(&len_bytes);

                                        // Commit the packet
                                        self.producer.commit(2 + packet_size, grant_w);
                                    } else {
                                        // Discard zero-length packet
                                        self.producer.commit(0, grant_w);
                                        self.state = PacketProcessorState::WaitingForGrant;
                                    }
                                } else {
                                    // Decoding error, discard the packet
                                    self.producer.commit(0, grant_w);
                                    self.state = PacketProcessorState::WaitingForGrant;
                                }
                            }
                        } else {
                            self.producer.commit(0, grant_w);
                            self.state = PacketProcessorState::Discarding;
                        }

                        // Input data chunk is either processed or discarded, release it
                        self.consumer.release(chunk_size, grant_r);
                    } else {
                        unreachable!();
                    }
                },
            };
        }
    }
}

pub struct PacketConsumerGrantR {
    grant: GrantR,
    packet_size: usize,
}

impl Deref for PacketConsumerGrantR {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.grant[2..2 + self.packet_size]
    }
}

pub struct PacketConsumer {
    consumer: Consumer,
}


impl PacketConsumer {
    pub fn new(consumer: Consumer) -> Self {
        Self {
            consumer
        }
    }

    pub fn read(&mut self) -> Option<PacketConsumerGrantR> {
        if let Ok(grant) = self.consumer.read() {
            if grant.len() >= 2 {
                let packet_size = u16::from_ne_bytes([grant[0], grant[1]]) as usize;
                if 2 + packet_size <= grant.len() {
                    return Some(PacketConsumerGrantR {
                        grant,
                        packet_size,
                    })
                }
            }

            self.consumer.release(0, grant);
        }
        None
    }

    pub fn release_consume(&mut self, grant: PacketConsumerGrantR) {
        self.consumer.release(2 + grant.packet_size, grant.grant);
    }

    pub fn release_unread(&mut self, grant: PacketConsumerGrantR) {
        self.consumer.release(0, grant.grant);
    }
}
