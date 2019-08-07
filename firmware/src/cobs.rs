enum DecoderState {
    Idle,
    Decoding(u8),
}

pub struct CobsDecoder {
    state: DecoderState
}

impl CobsDecoder {
    pub fn new() -> Self {
        Self {
            state: DecoderState::Idle
        }
    }

    /// Returns (raw size, data size, finished)
    pub fn decode(&mut self, buffer: &mut [u8]) -> (usize, usize, bool) {
        let mut read_idx = 0;
        let mut write_idx = 0;
        while read_idx < buffer.len() {
            let byte = buffer[read_idx];
            read_idx += 1;

            match self.state {
                DecoderState::Idle => {
                    if byte != 0 {
                        self.state = DecoderState::Decoding(byte - 1)
                    }
                },
                DecoderState::Decoding(b) if b == 0 => {
                    if byte == 0 {
                        self.state = DecoderState::Idle;
                        return (read_idx, write_idx, true);
                    } else {
                        self.state = DecoderState::Decoding(byte - 1);
                        buffer[write_idx] = 0;
                        write_idx += 1;
                    }
                },
                DecoderState::Decoding(b) => {
                    self.state = DecoderState::Decoding(b - 1);
                    buffer[write_idx] = byte;
                    write_idx += 1;
                },
            }
        }
        (read_idx, write_idx, false)
    }
}

pub fn cobs_encode_in_place(buffer: &mut [u8], data_offset: usize, data_size: usize) -> usize {
    let mut read_idx = data_offset;
    let mut write_idx = 1;
    let mut code = 1;
    let mut code_index = 0;
    while read_idx < data_offset + data_size {
        if code != 0xff {
            let byte = buffer[read_idx];
            read_idx += 1;
            if byte != 0 {
                buffer[write_idx] = byte;
                write_idx += 1;
                code += 1;
                continue;
            }
        }
        buffer[code_index] = code;
        code_index = write_idx;
        write_idx += 1;
        code = 1;
    }
    buffer[code_index] = code;
    write_idx
}
