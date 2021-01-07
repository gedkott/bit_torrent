use std::io::{Read, Write};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::thread;
use utp::UtpStream;

use crate::messages::*;
use crate::util::read_be_u32;
use crate::{HandshakeError, InterestedError};

impl crate::PeerStreamT for Stream {}

pub struct Stream {
    peer_id: Vec<u8>,
    utp_stream: UtpStream,
    am_interested: bool,
}

// TODO(): this is a hack because of UtpStreams being non-Copy to make sure we can print streams, but
// really we just need Stream to impl Debug to satisfy compiler...
impl std::fmt::Debug for Stream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Point")
            .field("peer_id", &self.peer_id)
            .finish()
    }
}

impl crate::PeerStream for Stream {
    fn handshake(&mut self, info_hash: &[u8]) -> Result<(), HandshakeError> {
        let handshake = Handshake {
            info_hash,
            peer_id: &self.peer_id,
        };
        let bytes: Vec<u8> = handshake.serialize();

        self.utp_stream
            .write_all(&bytes)
            .map_err(|_e| HandshakeError::TcpWrite)
            .and_then(move |_| {
                // handshake includes reading the return handshake
                let mut buf: [u8; 68] = [0; 68];

                match self.utp_stream.read_exact(&mut buf) {
                    Ok(_) => {
                        let m = Handshake::new(&buf);
                        println!("handshake from peer: {:?}", m);
                        Ok(())
                    }
                    Err(_e) => Err(HandshakeError::TcpReturnHandshakeRead(_e, buf)),
                }
            })
    }

    fn interested(&mut self) -> Result<(), InterestedError> {
        self.am_interested = true;
        let bytes: Vec<u8> = Message::Interested.serialize();

        println!("identifying as interested in peer: {:?}", self);

        self.utp_stream
            .write_all(&bytes)
            .map_err(|_e| InterestedError::TcpWrite)
    }
}

pub struct PeerUtpClient {
    pub connections: Vec<Stream>,
    pub info_hash: Vec<u8>,
    message_sender: crate::client::MessageSender,
    message_receiver: crate::client::MessageReceiver,
}

impl crate::PeerClient for PeerUtpClient {
    fn connect(peers: &[crate::tracker::Peer], info_hash: &[u8]) -> Self {
        let connections: Vec<Stream> = peers
            .iter()
            .filter_map(|p| {
                println!("connecting to peer {:?} over tcp", p);
                if let Ok(utp_stream) = UtpStream::connect(&p.socket_addr) {
                    println!("connected to peer over tcp");
                    Some(Stream {
                        peer_id: p.id.clone(),
                        utp_stream,
                        am_interested: false,
                    })
                } else {
                    println!("one of our peers didn't connect");
                    None
                }
            })
            .collect();

        let (sender, receiver) = channel();

        PeerUtpClient {
            connections,
            info_hash: info_hash.to_vec(),
            message_sender: sender,
            message_receiver: receiver,
        }
    }

    fn listen(self) -> crate::Readers {
        let r = self.message_receiver;
        let tx = self.message_sender;

        let threads = self
            .connections
            .into_iter()
            .map(|c| {
                let s_c = Arc::new(Mutex::new(c));
                let r_s_c = Arc::clone(&s_c);
                let tx = tx.clone();

                let thread_handle = thread::spawn(move || loop {
                    let mut buf = [0u8; 4].to_vec();
                    match s_c
                        .lock()
                        .unwrap()
                        .utp_stream
                        .read_exact(&mut buf)
                        .map_err(MessageParseError::PrefixLenRead)
                        .and_then(|_| {
                            let prefix_len = read_be_u32(&mut buf.as_slice())
                                .map_err(|_| MessageParseError::PrefixLenConvert)?;

                            println!("length of next message is {:?} from {:?}", prefix_len, buf);

                            let mut message_buf = Vec::with_capacity(prefix_len as usize);

                            s_c.lock()
                                .unwrap()
                                .utp_stream
                                .read_exact(&mut message_buf)
                                .map_err(|_| MessageParseError::MessageRead)
                                .map(|_| (prefix_len, message_buf))
                        })
                        .and_then(|(prefix_len, message_buf)| {
                            tx.send((
                                Arc::clone(&s_c) as Arc<Mutex<dyn crate::PeerStreamT>>,
                                Message::new(Box::new(message_buf.into_iter()), prefix_len),
                            ))
                            .map_err(|_| MessageParseError::SendError)
                        })
                        .map_err(|e| {
                            panic!("{:?}", e);
                        }) {
                        Ok(_) => (),
                        Err(e) => panic!("a client broke down: {:?}", e),
                    }
                });

                (thread_handle, r_s_c as Arc<Mutex<dyn crate::PeerStreamT>>)
            })
            .collect();

        crate::Readers {
            receiver: r,
            threads,
        }
    }
}
