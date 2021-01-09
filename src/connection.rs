use crate::messages::*;
use crate::util;
use std::io::prelude::*;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub enum SendError {
    HandshakeParse,
    Write(std::io::Error),
    ReturnHandshakeRead(std::io::Error),
    ReturnHandshakeReadTimeOut,
    Connect(std::io::Error),
}

pub enum Stream {
    Tcp(std::net::TcpStream),
}

pub enum StreamType {
    Tcp,
}

pub struct PeerConnection {
    stream: Arc<Mutex<Stream>>,
    pub stream_type: StreamType,
    is_peer_choked: bool,
    is_local_choked: bool,
    is_peer_interested: bool,
    is_local_interested: bool
}

impl PeerConnection {
    pub fn new(
        mut stream: Stream,
        info_hash: &[u8],
        my_peer_id: &[u8],
    ) -> Result<Self, SendError> {
        let stream_type: StreamType = match stream {
            Stream::Tcp(_) => StreamType::Tcp,
        };

        let handshake = Handshake {
            info_hash: info_hash.to_vec(),
            peer_id: my_peer_id.to_vec(),
        };
        let bytes: Vec<u8> = handshake.serialize();

        let write_result = {
            println!("writing handshake frame: {:?}", handshake);
            stream.write_all(&bytes).map_err(|e| SendError::Write(e))
        };

        let arc_stream = Arc::new(Mutex::new(stream));
        let thread_stream = Arc::clone(&arc_stream);
        
        write_result.and_then(|_| {
                // handshake includes reading the return handshake
                let work = Box::new(move || {
                    let mut buf: Vec<u8> = vec![0; 68];
                    println!("reading handshake frame");
                    match thread_stream.lock().unwrap().read_exact(&mut buf) {
                        Ok(_) => {
                            println!("succesfully read handshake frame");
                            Ok(buf)
                        }
                        Err(e) => {
                            println!("failed to read handshake frame");
                            Err(SendError::ReturnHandshakeRead(e))
                        }
                    }
                });

                println!("about to race stream read");

                util::with_timeout(work, std::time::Duration::from_millis(1500)).map_err(|e| {
                    println!("timeout or execution error {:?}", e);
                    match e {
                        crate::util::ExecutionErr::TimedOut => {
                            SendError::ReturnHandshakeReadTimeOut
                        }
                        crate::util::ExecutionErr::Err(e) => e,
                    }
                })
            })
            .and_then(|buf| Handshake::new(&buf).map_err(|_| SendError::HandshakeParse))
            .map(|_| PeerConnection {
                stream: arc_stream,
                stream_type,
                is_local_choked: true,
                is_peer_choked: true,
                is_local_interested: false,
                is_peer_interested: false
            })
    }

    pub fn write_message(&mut self, m: Message) -> Result<(), SendError> {
        match m {
            _ => {
                let bytes: Vec<u8> = m.serialize();

                let write_result = {
                    println!("writing message: {:?}", m);
                    self.stream
                        .lock()
                        .unwrap()
                        .write_all(&bytes)
                        .map_err(|e| SendError::Write(e))
                };

                write_result
            },
        }
    }

    pub fn read_message(&mut self) -> Result<Message, MessageParseError> {
        let mut buf = [0u8; 4].to_vec();

        let read_prefix_len_result = {
            self.stream
                .lock()
                .unwrap()
                .read_exact(&mut buf)
                .map_err(MessageParseError::PrefixLenRead)
        };

        read_prefix_len_result
            .and_then(|_| {
                let prefix_len = util::read_be_u32(&mut buf.as_slice())
                    .map_err(|_| MessageParseError::PrefixLenConvert)?;
                let mut message_buf = vec![0u8; prefix_len as usize];
                if prefix_len == 0 {
                    // TODO(): keep-alive messages do not need to read bytes before parsing
                    Ok((vec![], 0))
                } else {
                    self.stream
                        .lock()
                        .unwrap()
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
