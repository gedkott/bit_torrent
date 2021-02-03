use crate::messages::*;
use crate::util;
use crate::BitField;
use std::io::prelude::*;

#[derive(Debug)]
pub enum SendError {
    HandshakeParse,
    Write(std::io::Error),
    ReturnHandshakeRead(std::io::Error),
    ReturnHandshakeReadTimeOut,
    Connect(std::io::Error),
}

#[derive(Debug)]
pub enum Stream {
    Tcp(std::net::TcpStream),
}

pub enum StreamType {
    Tcp,
}

pub struct PeerConnection {
    pub stream: Stream,
    pub stream_type: StreamType,
    _is_peer_choked: bool,
    pub is_local_choked: bool,
    _is_peer_interested: bool,
    pub is_local_interested: bool,
    pub bitfield: Option<BitField>,
}

impl PeerConnection {
    pub fn new(mut stream: Stream, info_hash: &[u8], my_peer_id: &[u8]) -> Result<Self, SendError> {
        let stream_type: StreamType = match stream {
            Stream::Tcp(_) => StreamType::Tcp,
        };

        let handshake = Handshake {
            info_hash: info_hash.to_vec(),
            peer_id: my_peer_id.to_vec(),
        };
        let bytes: Vec<u8> = handshake.serialize();

        let write_result = stream.write_all(&bytes).map_err(SendError::Write);

        write_result
            .and_then(|_| {
                let work = Box::new(move || {
                    let mut buf: Vec<u8> = vec![0; 68];
                    match stream.read_exact(&mut buf) {
                        Ok(_) => Ok((buf, stream)),
                        Err(e) => Err(SendError::ReturnHandshakeRead(e)),
                    }
                });

                util::with_timeout(work, std::time::Duration::from_millis(1500)).map_err(
                    |e| match e {
                        crate::util::ExecutionErr::TimedOut => {
                            SendError::ReturnHandshakeReadTimeOut
                        }
                        crate::util::ExecutionErr::Err(e) => e,
                    },
                )
            })
            .and_then(|(buf, stream)| {
                Handshake::new(&buf)
                    .map_err(|_| SendError::HandshakeParse)
                    .map(|_| stream)
            })
            .map(|s| PeerConnection {
                stream: s,
                stream_type,
                is_local_choked: true,
                _is_peer_choked: true,
                is_local_interested: false,
                _is_peer_interested: false,
                bitfield: None,
            })
    }

    pub fn write_message(&mut self, m: Message) -> Result<(), SendError> {
        let bytes: Vec<u8> = m.serialize();

        self.stream.write_all(&bytes).map_err(SendError::Write)
    }

    pub fn read_message(&mut self) -> Result<Message, MessageParseError> {
        let mut buf = [0u8; 4].to_vec();

        self.stream
            .read_exact(&mut buf)
            .map_err(MessageParseError::PrefixLenRead)
            .and_then(|_| {
                let prefix_len = util::read_be_u32(&mut buf.as_slice())
                    .map_err(|_| MessageParseError::PrefixLenConvert)?;
                let mut message_buf = vec![0u8; prefix_len as usize];
                if prefix_len == 0 {
                    Ok((vec![], 0))
                } else {
                    self.stream
                        .read_exact(&mut message_buf)
                        .map_err(|_| MessageParseError::MessageRead)
                        .map(|_| (message_buf, prefix_len))
                }
            })
            .and_then(|(message_buf, prefix_len)| {
                Message::new(Box::new(message_buf.into_iter()), prefix_len)
            })
    }
}

impl std::io::Write for Stream {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        match self {
            Stream::Tcp(ts) => ts.write(buf),
        }
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        match self {
            Stream::Tcp(ts) => ts.flush(),
        }
    }
}

impl std::io::Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        match self {
            Stream::Tcp(ts) => ts.read(buf),
        }
    }
}
