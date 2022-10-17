#[derive(Debug)]
pub struct BitField {
    bf: Vec<u8>,
}

#[derive(Debug, PartialEq)]
pub enum BitFieldError {
    InvalidBit(usize),
}

impl BitField {
    pub fn is_set(&self, bit: usize) -> Result<bool, BitFieldError> {
        let byte = bit / 8;
        let offset_in_byte = bit % 8;
        match self.bf.get(byte) {
            Some(byte) => {
                let left_shifted = 1 << (7 - offset_in_byte);
                Ok((left_shifted & byte) != 0)
            }
            None => Err(BitFieldError::InvalidBit(bit)),
        }
    }

    pub fn set(&mut self, bit: usize) {
        let byte = bit / 8;
        let offset_in_byte = bit % 8;
        if let Some(byte) = self.bf.get_mut(byte) {
            let left_shifted = 1 << (7 - offset_in_byte);
            *byte |= left_shifted;
        };
    }
}

impl From<Vec<u8>> for BitField {
    fn from(bf: Vec<u8>) -> BitField {
        BitField { bf }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_can_use_a_predefined_bitfield() {
        let bitfield: BitField = vec![1, 3, 5, 7].into();
        // [ [0..1], [0..1, 1], [0..1, 0, 1], [0.. 1, 1, 1]  ]

        for bit in &[7, 14, 15, 21, 23, 29, 30, 31] {
            assert_eq!(Ok(true), bitfield.is_set(*bit));
        }
    }

    #[test]
    fn it_can_set_a_bit_in_existing_bitfield() {
        let mut bitfield: BitField = vec![192].into();
        // [ [1, 1, 0]  ]

        for bit in &[0, 1] {
            assert_eq!(Ok(true), bitfield.is_set(*bit));
        }

        for bit in &[2] {
            assert_eq!(Ok(false), bitfield.is_set(*bit));
            bitfield.set(*bit);
            assert_eq!(Ok(true), bitfield.is_set(*bit));
        }
    }
}
