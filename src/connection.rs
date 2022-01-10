use crate::messages::*;
use crate::util;
use crate::util::ExecutionErr;
use crate::BitField;
use std::io::prelude::*;
use std::io::Error as IOError;
use std::net::SocketAddr;
use std::net::TcpStream;
use std::time::Duration;

#[derive(Debug)]
pub enum SendError {
    HandshakeParse,
    Write(IOError),
    ReturnHandshakeRead(IOError),
    ReturnHandshakeReadTimeOut,
    Connect(IOError),
    UnexpectedInfoHashOrPeerId,
}

#[derive(Debug)]
pub enum Stream {
    Tcp(TcpStream),
}

pub struct PeerConnection {
    stream: Stream,
    pub is_local_interested: bool,
    pub is_choked: bool,
    pub bitfield: Option<BitField>,
    pub peer_addr: std::net::SocketAddr,
    pub local_addr: std::net::SocketAddr,
    pub in_progress_requests: usize,
    on_read: Box<dyn Fn((crate::Message, SocketAddr, SocketAddr), &[u8]) -> () + 'static + Send>,
}

const HANDSHAKE_READ_TIMEOUT: Duration = Duration::from_millis(1500);

impl PeerConnection {
    pub fn new(
        mut stream: Stream,
        info_hash: &[u8],
        my_peer_id: &[u8],
        peer_id: &[u8],
        on_read: Box<
            dyn Fn((crate::Message, SocketAddr, SocketAddr), &[u8]) -> () + 'static + Send,
        >,
    ) -> Result<Self, SendError> {
        let handshake = Handshake {
            info_hash: info_hash.to_vec(),
            peer_id: my_peer_id.to_vec(),
        };
        let bytes: Vec<u8> = handshake.serialize();

        stream
            .write_all(&bytes)
            .map_err(SendError::Write)
            .and_then(|_| {
                let work = move || {
                    let mut buf: Vec<u8> = vec![0; 68];
                    stream
                        .read_exact(&mut buf)
                        .map(|_| (buf, stream))
                        .map_err(SendError::ReturnHandshakeRead)
                };

                util::with_timeout(work, HANDSHAKE_READ_TIMEOUT).map_err(|e| match e {
                    ExecutionErr::TimedOut => SendError::ReturnHandshakeReadTimeOut,
                    ExecutionErr::Err(e) => e,
                })
            })
            .and_then(|(buf, stream)| {
                Handshake::new(&buf)
                    .map_err(|_| SendError::HandshakeParse)
                    .and_then(|return_handshake| {
                        if handshake.info_hash == return_handshake.info_hash && return_handshake.peer_id == peer_id {
                            Ok(stream)
                        } else {
                            // println!("outgoing handshake: {:?}\nincoming handshake: {:?}\nexpected peer id: {:?}", handshake, return_handshake, peer_id);
                            Err(SendError::UnexpectedInfoHashOrPeerId)
                        }
                    })
            })
            .map(|s| {
                let peer_addr = match &s {
                    Stream::Tcp(tcps) => tcps.peer_addr().unwrap()
                };
                let local_addr = match &s {
                    Stream::Tcp(tcps) => tcps.local_addr().unwrap()
                };
                PeerConnection {
                    stream: s,
                    is_local_interested: false,
                    is_choked: true,
                    bitfield: None,
                    peer_addr,
                    local_addr,
                    in_progress_requests: 0,
                    on_read: Box::new(on_read),
                }
            })
    }

    pub fn write_message(&mut self, m: Message) -> Result<(), SendError> {
        let to_write = &m.serialize();
        (self.on_read)((m, self.peer_addr, self.local_addr), to_write);
        self.stream.write_all(to_write).map_err(SendError::Write)
    }

    pub fn read_message(&mut self) -> Result<Message, MessageParseError> {
        let mut buf = [0u8; 4].to_vec();

        self.stream
            .read_exact(&mut buf)
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::ConnectionRefused => MessageParseError::ConnectionRefused,
                std::io::ErrorKind::ConnectionReset => MessageParseError::ConnectionReset,
                std::io::ErrorKind::ConnectionAborted => MessageParseError::ConnectionAborted,
                std::io::ErrorKind::WouldBlock => MessageParseError::WouldBlock,
                std::io::ErrorKind::TimedOut => MessageParseError::TimedOut,
                std::io::ErrorKind::WriteZero => MessageParseError::WriteZero,
                std::io::ErrorKind::Interrupted => MessageParseError::Interrupted,
                std::io::ErrorKind::UnexpectedEof => MessageParseError::UnexpectedEof,
                _ => MessageParseError::WildWildWest,
            })
            .and_then(|_| {
                let prefix_len = util::read_be_u32(&mut buf.as_slice())
                    .map_err(|_| MessageParseError::PrefixLenConvert)?;
                if prefix_len == 0 {
                    Ok((vec![], 0))
                } else {
                    let mut message_buf = vec![0u8; prefix_len as usize];
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
    fn write(&mut self, buf: &[u8]) -> Result<usize, IOError> {
        match self {
            Stream::Tcp(ts) => ts.write(buf),
        }
    }

    fn flush(&mut self) -> Result<(), IOError> {
        match self {
            Stream::Tcp(ts) => ts.flush(),
        }
    }
}

impl std::io::Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, IOError> {
        match self {
            Stream::Tcp(ts) => ts.read(buf),
        }
    }
}
