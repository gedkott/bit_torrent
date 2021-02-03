#[derive(Debug)]
pub struct BitField {
    bf: Vec<u8>,
    len: usize,
}

#[derive(Debug, PartialEq)]
pub enum BitFieldError {
    InvalidBit(usize),
}

impl BitField {
    pub fn _new(size: usize) -> BitField {
        // size is the number of bits
        // Math.ceil(size / 8) is the number of bytes we need to store the intended bitmap
        // TODO(): the bits available can be more than the user needs
        let full_size = (size as f32 / 8f32).ceil();
        BitField {
            bf: vec![0; full_size as usize],
            len: size,
        }
    }

    pub fn _set(&mut self, bit: usize) -> Result<(), BitFieldError> {
        let byte = bit / 8;
        let offset_in_byte = bit % 8;
        let updated_byte = match self.bf.get(byte) {
            Some(byte) => {
                // set(2)
                // [0 1 0 1 0 1 0 1]
                //      ^
                // shift a 1 left by ( 7 - offset_in_byte )
                // [0 0 0 0 0 0 0 1] << 7 - 2
                // [0 0 0 0 0 0 0 1] << 5
                // [0 0 1 0 0 0 0 0]

                // bitwise OR with original value
                // [0 0 1 0 0 0 0 0]
                // ^
                // [0 1 0 1 0 1 0 1]
                // [0 1 1 1 0 1 0 1]
                let left_shifted = 1 << (7 - offset_in_byte);
                left_shifted | byte
            }
            None => return Err(BitFieldError::InvalidBit(bit)),
        };
        self.bf[byte] = updated_byte;
        Ok(())
    }

    pub fn is_set(&self, bit: usize) -> Result<bool, BitFieldError> {
        let byte = bit / 8;
        let offset_in_byte = bit % 8;
        match self.bf.get(byte) {
            Some(byte) => {
                /*
                AND together with zero-testing can be used to determine if a bit is set:
                    11101010 AND 00000001 = 00000000 = 0 means the bit at the 7th LSB ISN'T set
                    11101010 AND 00000010 = 00000010 ≠ 0 means the bit at the 7th LSB IS set
                */
                // is_set(2)
                // [0 1 0 1 0 1 0 1]
                //      ^
                // shift a 1 left by ( 7 - offset_in_byte )
                // [0 0 0 0 0 0 0 1] << 7 - 2
                // [0 0 0 0 0 0 0 1] << 5
                // [0 0 1 0 0 0 0 0]

                // bitwise AND with original value
                // [0 0 1 0 0 0 0 0]
                // &
                // [0 1 0 1 0 1 0 1]
                // [0 0 0 0 0 0 0 0]

                // is result zero? if so, the bit ISN'T set, else, the bit IS set
                // [0 0 0 0 0 0 0 0] == 0
                let left_shifted = 1 << (7 - offset_in_byte);
                Ok((left_shifted & byte) != 0)
            }
            None => Err(BitFieldError::InvalidBit(bit)),
        }
    }
}

impl From<Vec<u8>> for BitField {
    fn from(bf: Vec<u8>) -> BitField {
        let len = bf.len();
        BitField { bf, len }
    }
}

impl std::iter::IntoIterator for BitField {
    type Item = u8;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.bf.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_can_set_bits_by_index() {
        let mut bitfield = BitField::_new(8);

        for b in 0..bitfield.len {
            if b % 2 == 0 {
                bitfield._set(b).unwrap();
            } else {
            }
        }

        for b in 0..bitfield.len {
            if b % 2 == 0 {
                assert_eq!(Ok(true), bitfield.is_set(b));
            } else {
                assert_eq!(Ok(false), bitfield.is_set(b));
            }
        }
    }

    #[test]
    fn it_can_use_a_predefined_bitfield() {
        let bitfield: BitField = vec![1, 3, 5, 7].into();
        // [ [0..1], [0..1, 1], [0..1, 0, 1], [0.. 1, 1, 1]  ]

        for bit in &[7, 14, 15, 21, 23, 29, 30, 31] {
            assert_eq!(Ok(true), bitfield.is_set(*bit));
        }
    }
}
