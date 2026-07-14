use super::{KlfError, Result};

const END: u8 = 0xC0;
const ESC: u8 = 0xDB;
const ESC_END: u8 = 0xDC;
const ESC_ESC: u8 = 0xDD;
const DEFAULT_MAX_FRAME_LENGTH: usize = 1024;

#[must_use]
pub fn encode(frame: &[u8]) -> Vec<u8> {
    let escaped_bytes = frame.iter().filter(|&&byte| matches!(byte, END | ESC)).count();
    let mut encoded = Vec::with_capacity(frame.len() + escaped_bytes + 2);
    encoded.push(END);
    for byte in frame {
        match *byte {
            END => encoded.extend_from_slice(&[ESC, ESC_END]),
            ESC => encoded.extend_from_slice(&[ESC, ESC_ESC]),
            byte => encoded.push(byte),
        }
    }
    encoded.push(END);
    encoded
}

#[derive(Debug)]
pub struct Decoder {
    buffer: Vec<u8>,
    maximum_length: usize,
    started: bool,
    escaped: bool,
    discarding: bool,
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_FRAME_LENGTH)
    }
}

impl Decoder {
    #[must_use]
    pub fn new(maximum_length: usize) -> Self {
        Self {
            buffer: Vec::new(),
            maximum_length,
            started: false,
            escaped: false,
            discarding: false,
        }
    }

    pub fn push(&mut self, input: &[u8]) -> Vec<Result<Vec<u8>>> {
        let mut decoded = Vec::new();
        for &byte in input {
            if byte == END {
                if !self.started {
                    self.started = true;
                } else if self.escaped && !self.discarding {
                    decoded.push(Err(KlfError::IncompleteSlipEscape));
                } else if !self.discarding && !self.buffer.is_empty() {
                    decoded.push(Ok(std::mem::take(&mut self.buffer)));
                }
                self.buffer.clear();
                self.escaped = false;
                self.discarding = false;
                continue;
            }

            if !self.started || self.discarding {
                continue;
            }

            let decoded_byte = if self.escaped {
                self.escaped = false;
                match byte {
                    ESC_END => END,
                    ESC_ESC => ESC,
                    byte => {
                        decoded.push(Err(KlfError::InvalidSlipEscape { byte }));
                        self.buffer.clear();
                        self.discarding = true;
                        continue;
                    }
                }
            } else if byte == ESC {
                self.escaped = true;
                continue;
            } else {
                byte
            };

            if self.buffer.len() == self.maximum_length {
                decoded.push(Err(KlfError::SlipFrameTooLarge {
                    maximum: self.maximum_length,
                }));
                self.buffer.clear();
                self.discarding = true;
            } else {
                self.buffer.push(decoded_byte);
            }
        }
        decoded
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn escapes_reserved_bytes() {
        assert_eq!(encode(&[END, ESC, 1]), [END, ESC, ESC_END, ESC, ESC_ESC, 1, END]);
    }

    #[test]
    fn handles_fragmented_and_coalesced_frames() {
        let first = encode(&[1, END, 2]);
        let second = encode(&[3, ESC, 4]);
        let stream = [first, second].concat();
        let mut decoder = Decoder::default();
        let mut frames = Vec::new();
        for chunk in stream.chunks(2) {
            frames.extend(decoder.push(chunk));
        }
        assert_eq!(frames, [Ok(vec![1, END, 2]), Ok(vec![3, ESC, 4])]);
    }

    #[test]
    fn ignores_noise_empty_frames_and_incomplete_input() {
        let mut decoder = Decoder::default();
        assert!(decoder.push(&[1, 2, 3]).is_empty());
        assert!(decoder.push(&[END, END, END]).is_empty());
        assert!(decoder.push(&[1, 2]).is_empty());
        assert_eq!(decoder.push(&[END]), [Ok(vec![1, 2])]);
    }

    #[test]
    fn reports_malformed_input_and_recovers_at_the_next_boundary() {
        let mut decoder = Decoder::new(2);
        assert_eq!(
            decoder.push(&[END, ESC, 0x42]),
            [Err(KlfError::InvalidSlipEscape { byte: 0x42 })]
        );
        assert_eq!(decoder.push(&[9, END, 1, 2, END]), [Ok(vec![1, 2])]);
        assert_eq!(
            decoder.push(&[1, 2, 3]),
            [Err(KlfError::SlipFrameTooLarge { maximum: 2 })]
        );
        assert!(decoder.push(&[END]).is_empty());
        assert_eq!(decoder.push(&[ESC, END]), [Err(KlfError::IncompleteSlipEscape)]);
    }

    proptest! {
        #[test]
        fn arbitrary_frames_round_trip(frame in prop::collection::vec(any::<u8>(), 1..512)) {
            let mut decoder = Decoder::default();
            let decoded = decoder.push(&encode(&frame));
            prop_assert_eq!(decoded, vec![Ok(frame)]);
        }

        #[test]
        fn arbitrary_fragmentation_round_trips(
            frame in prop::collection::vec(any::<u8>(), 1..512),
            chunk_size in 1usize..32,
        ) {
            let encoded = encode(&frame);
            let mut decoder = Decoder::default();
            let mut decoded = Vec::new();
            for chunk in encoded.chunks(chunk_size) {
                decoded.extend(decoder.push(chunk));
            }
            prop_assert_eq!(decoded, vec![Ok(frame)]);
        }
    }
}
