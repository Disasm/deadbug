use log::info;
use crate::cobs_tx::{CobsTxProducer, CobsTxGrantW};
use crate::packet_processor::{PacketConsumer, PacketConsumerGrantR};
use deadbug_common::hal::{HalError, HalResult, HalErrorKind};
use core::ops::{Deref, DerefMut};
use crate::targets::{BoardGpioPinSet, BoardGpioPin};
use deadbug_common::hal::gpio::GpioPin;


enum CommandError {
    NeedWriteGrant(usize),
    Hal(HalError),
}

impl From<HalError> for CommandError {
    fn from(e: HalError) -> Self {
        CommandError::Hal(e)
    }
}

pub struct CommandProcessor {
    producer: CobsTxProducer,
    consumer: PacketConsumer,
    write_grant_request: Option<usize>,
    gpio_target: GpioCommandTarget,
}

impl CommandProcessor {
    pub fn new(producer: CobsTxProducer, consumer: PacketConsumer, gpio_target: GpioCommandTarget) -> Self {
        Self {
            producer,
            consumer,
            write_grant_request: None,
            gpio_target
        }
    }

    #[inline(never)]
    pub fn process(&mut self) {
        if let Some(read_grant) = self.consumer.read() {
            info!("got grant, len {}", read_grant.len());

            if read_grant.len() < 2 {
                // Invalid packet
                self.consumer.release_consume(read_grant);
                return;
            }

            let write_grant_size = self.write_grant_request.unwrap_or(4);
            if let Some(mut write_grant) = self.producer.grant(write_grant_size) {
                let target = read_grant[0];
                let read_grant_shim = CommandGrantR(&read_grant);
                let write_grant_shim = CommandGrantW(&mut write_grant);
                match self.process_command(target, read_grant_shim, write_grant_shim) {
                    Ok(size) => {
                        write_grant[0] = 0;
                        self.producer.commit_with_size(1 + size, write_grant);
                        self.consumer.release_consume(read_grant);
                        self.write_grant_request = None;
                    },
                    Err(CommandError::NeedWriteGrant(size)) => {
                        self.write_grant_request = Some(size);
                        self.producer.commit_with_size(0, write_grant);
                        self.consumer.release_unread(read_grant);
                    },
                    Err(CommandError::Hal(e)) => {
                        write_grant[0] = 0xff;
                        let size = ssmarshal::serialize(&mut write_grant[1..], &e.kind()).unwrap();
                        self.producer.commit_with_size(1 + size, write_grant);
                        self.consumer.release_consume(read_grant);
                        self.write_grant_request = None;
                    },
                }
            } else {
                self.consumer.release_unread(read_grant);
                return;
            }
        }
    }

    fn process_command(&mut self, target: u8, read_grant: CommandGrantR, write_grant: CommandGrantW) -> Result<usize, CommandError> {
        if target == 1 {
            return self.gpio_target.process_command(read_grant, write_grant);
        }
        Err(CommandError::Hal(HalErrorKind::UnsupportedCommand.into()))
    }
}

struct CommandGrantR<'a>(&'a PacketConsumerGrantR);

impl Deref for CommandGrantR<'_> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0[1..]
    }
}

struct CommandGrantW<'a>(&'a mut CobsTxGrantW);

impl CommandGrantW<'_> {
    pub fn check_size(&self, size: usize) -> Result<(), CommandError> {
        if size <= (self.0.len() - 1) {
            Ok(())
        } else {
            Err(CommandError::NeedWriteGrant(size))
        }
    }
}

impl Deref for CommandGrantW<'_> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0[1..]
    }
}

impl DerefMut for CommandGrantW<'_> {
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut self.0[1..]
    }
}

trait CommandTarget {
    fn get_descriptor(&self) -> u8;

    fn process_command(&mut self, read_grant: CommandGrantR, write_grant: CommandGrantW) -> Result<usize, CommandError>;
}

pub struct GpioCommandTarget {
    pins: BoardGpioPinSet,
}

impl GpioCommandTarget {
    pub fn new(pins: BoardGpioPinSet) -> Self {
        Self {
            pins
        }
    }

    fn pin(&self, index: u8) -> HalResult<&BoardGpioPin> {
        (&self.pins).into_iter().nth(index as usize).ok_or_else(|| HalErrorKind::InvalidParameter.into())
    }

    fn pin_mut(&mut self, index: u8) -> HalResult<&mut BoardGpioPin> {
        (&mut self.pins).into_iter().nth(index as usize).ok_or_else(|| HalErrorKind::InvalidParameter.into())
    }
}

impl CommandTarget for GpioCommandTarget {
    fn get_descriptor(&self) -> u8 {
        0
    }

    fn process_command(&mut self, read_grant: CommandGrantR, mut write_grant: CommandGrantW) -> Result<usize, CommandError> {
        use deadbug_common::protocol::GpioCommand;

        let command: GpioCommand = ssmarshal::deserialize(&read_grant).map_err(|e| HalError::from(e))?.0;
        info!("command: {:?}", command);
        match command {
            GpioCommand::EnumeratePins => {
                Err(HalError::from(HalErrorKind::UnsupportedCommand))
            },
            GpioCommand::GetPinMode(index) => {
                write_grant.check_size(2)?;
                let pin = self.pin(index)?;
                let mode = pin.mode();
                let size = ssmarshal::serialize(&mut write_grant, &mode).unwrap();
                Ok(size)
            },
            GpioCommand::SetPinMode(index, mode) => {
                let pin = self.pin_mut(index)?;
                pin.set_mode(mode)?;
                Ok(0)
            },
            GpioCommand::SetPinValue(index, value) => {
                let pin = self.pin_mut(index)?;
                pin.set_output(value)?;
                Ok(0)
            },
            GpioCommand::GetPinValue(index) => {
                write_grant.check_size(1)?;
                let pin = self.pin(index)?;
                let value = pin.get_input()?;
                write_grant[0] = value as u8;
                Ok(1)
            },
        }.map_err(|e| e.into())
    }
}
