use bbqueue::{Producer, GrantW};
use core::ops::{Deref, DerefMut};
use crate::cobs::cobs_encode_in_place;

pub struct CobsTxGrantW {
    data_grant: GrantW,
    offset: usize,
}

impl Deref for CobsTxGrantW {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.data_grant[self.offset..]
    }
}

impl DerefMut for CobsTxGrantW {
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut self.data_grant[self.offset..]
    }
}

pub struct CobsTxProducer {
    data_producer: Producer,
}

impl CobsTxProducer {
    pub fn new(data_producer: Producer) -> Self {
        Self {
            data_producer,
        }
    }

    pub fn grant(&mut self, size: usize) -> Option<CobsTxGrantW> {
        let overhead = (size + 253) / 254 + 1;
        if let Ok(data_grant) = self.data_producer.grant(size + overhead) {
            Some(CobsTxGrantW {
                data_grant,
                offset: overhead
            })
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn commit(&mut self, grant: CobsTxGrantW) {
        let data_size = grant.data_grant.len() - grant.offset;
        self.commit_with_size_unchecked(data_size, grant)
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub fn commit_with_size(&mut self, size: usize, grant: CobsTxGrantW) {
        assert!((size + grant.offset) < grant.data_grant.len());
        self.commit_with_size_unchecked(size, grant)
    }

    #[inline(always)]
    fn commit_with_size_unchecked(&mut self, size: usize, mut grant: CobsTxGrantW) {
        let encoded_size = cobs_encode_in_place(&mut grant.data_grant, grant.offset, size);
        self.data_producer.commit(encoded_size, grant.data_grant);
    }
}
