use rand::Rng;
use std::convert::TryInto;

pub fn read_be_u32(input: &mut &[u8]) -> Result<u32, std::array::TryFromSliceError> {
    let (int_bytes, rest) = input.split_at(std::mem::size_of::<u32>());
    *input = rest;
    int_bytes.try_into().map(u32::from_be_bytes)
}

pub fn attach_bytes(bytes: &[std::slice::Iter<'_, u8>]) -> Vec<u8> {
    bytes.iter().cloned().flatten().cloned().collect()
}

pub fn random_string() -> String {
    rand::thread_rng().gen_ascii_chars().take(20).collect()
}
