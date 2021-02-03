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
}

impl From<Vec<u8>> for BitField {
    fn from(bf: Vec<u8>) -> BitField {
        let len = bf.len();
        BitField { bf, len }
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
}
