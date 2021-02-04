use std::convert::TryInto;

use crate::util::{attach_bytes, read_be_u32};

const P_STR_LEN: u8 = 19;
const P_STR: &str = "BitTorrent protocol";
const RESERVED_BYTES: [u8; 8] = [0; 8];

#[derive(Debug)]
pub struct Handshake {
    pub info_hash: Vec<u8>,
    pub peer_id: Vec<u8>,
}

#[derive(Debug)]
pub enum HandshakeParseError {
    PStrLen,
    PStr,
    ReservedBytes,
    InfoHash,
    PeerId,
}

#[derive(Debug)]
pub enum Message {
    KeepAlive,
    Choke,
    UnChoke,
    Interested,
    NotInterested,
    Have {
        index: u32,
    },
    BitField(Vec<u8>),
    Request {
        index: u32,
        begin: u32,
        length: u32,
    },
    Piece {
        index: u32,
        offset: u32,
        data: Vec<u8>,
    },
}

#[derive(Debug)]
pub enum MessageParseError {
    MessageRead,
    PrefixLenRead(std::io::Error),
    PrefixLenConvert,
    Id(u8),
    IdMissing,
    Have,
    Unimplemented(&'static str),
    Piece,
}

impl Message {
    pub fn serialize(&self) -> Vec<u8> {
        match self {
            Message::KeepAlive => attach_bytes(&[0u32.to_be_bytes().iter()]),
            Message::Choke => attach_bytes(&[1u32.to_be_bytes().iter(), 0u8.to_be_bytes().iter()]),
            Message::UnChoke => {
                attach_bytes(&[1u32.to_be_bytes().iter(), 1u8.to_be_bytes().iter()])
            }
            Message::Interested => {
                attach_bytes(&[1u32.to_be_bytes().iter(), 2u8.to_be_bytes().iter()])
            }
            Message::NotInterested => {
                attach_bytes(&[1u32.to_be_bytes().iter(), 3u8.to_be_bytes().iter()])
            }
            Message::Have { index } => attach_bytes(&[
                5u32.to_be_bytes().iter(),
                4u8.to_be_bytes().iter(),
                index.to_be_bytes().iter(),
            ]),
            Message::BitField(bf) => {
                let l = bf.len();
                let prefix_len = (l + 1) as u32;
                attach_bytes(&[
                    prefix_len.to_be_bytes().iter(),
                    5u8.to_be_bytes().iter(),
                    bf.iter(),
                ])
            }
            Message::Request {
                index,
                begin,
                length,
            } => attach_bytes(&[
                13u32.to_be_bytes().iter(),
                6u8.to_be_bytes().iter(),
                index.to_be_bytes().iter(),
                begin.to_be_bytes().iter(),
                length.to_be_bytes().iter(),
            ]),
            Message::Piece {
                index,
                offset,
                data,
            } => attach_bytes(&[
                (data.len() + 3).to_be_bytes().iter(),
                7u8.to_be_bytes().iter(),
                index.to_be_bytes().iter(),
                offset.to_be_bytes().iter(),
                data.iter(),
            ]),
        }
    }

    pub fn new(
        mut bytes: Box<dyn Iterator<Item = u8>>,
        prefix_len: u32,
    ) -> Result<Self, MessageParseError> {
        if prefix_len == 0 {
            Ok(Message::KeepAlive)
        } else {
            let id = bytes.next().ok_or(MessageParseError::IdMissing)?;

            match id {
                0 => Ok(Message::Choke),
                1 => Ok(Message::UnChoke),
                2 => Ok(Message::Interested),
                3 => Ok(Message::NotInterested),
                4 => {
                    let b: Vec<u8> = bytes.by_ref().take(4).collect();
                    let index =
                        read_be_u32(&mut b.as_slice()).map_err(|_| MessageParseError::Have)?;

                    Ok(Message::Have { index })
                }
                5 => {
                    let bitfield_len = prefix_len - 1;
                    Ok(Message::BitField(
                        bytes.take(bitfield_len as usize).collect(),
                    ))
                }
                // request
                6 => Err(MessageParseError::Unimplemented("6 - request")),
                // piece
                7 => {
                    let b: Vec<u8> = bytes.by_ref().take(4).collect();
                    let index =
                        read_be_u32(&mut b.as_slice()).map_err(|_| MessageParseError::Piece)?;

                    let b: Vec<u8> = bytes.by_ref().take(4).collect();
                    let offset =
                        read_be_u32(&mut b.as_slice()).map_err(|_| MessageParseError::Piece)?;

                    let data_block_len = prefix_len - 9;
                    Ok(Message::Piece {
                        index,
                        offset,
                        data: bytes.take(data_block_len as usize).collect(),
                    })
                }
                // cancel
                8 => Err(MessageParseError::Unimplemented("8 - cancel")),
                _ => Err(MessageParseError::Id(id)),
            }
        }
    }
}

impl Handshake {
    pub fn serialize(&self) -> Vec<u8> {
        [
            u8::to_be_bytes(P_STR_LEN).to_vec(),
            P_STR.as_bytes().to_vec(),
            RESERVED_BYTES.to_vec(),
            self.info_hash.to_vec(),
            self.peer_id.to_vec(),
        ]
        .iter()
        .flatten()
        .cloned()
        .collect()
    }

    pub fn new(bytes: &[u8]) -> Result<Handshake, HandshakeParseError> {
        let p_str_len: usize = (*bytes.get(0).ok_or(HandshakeParseError::PStrLen)?)
            .try_into()
            .map_err(|_| HandshakeParseError::PStrLen)?;

        let len: usize = 1 + p_str_len;

        let _p_str = bytes
            .get(1..len)
            .ok_or(HandshakeParseError::PStr)
            .and_then(|s| std::str::from_utf8(s).map_err(|_| HandshakeParseError::PStr))?;

        let _reserved_bytes = bytes
            .get(len..len + 8)
            .ok_or(HandshakeParseError::ReservedBytes)?;

        let info_hash = bytes
            .get(len + 8..len + 8 + 20)
            .ok_or(HandshakeParseError::InfoHash)?;

        let peer_id = bytes
            .get(len + 8 + 20..len + 8 + 20 + 20)
            .ok_or(HandshakeParseError::PeerId)?;

        Ok(Handshake {
            info_hash: info_hash.to_vec(),
            peer_id: peer_id.to_vec(),
        })
    }
}
