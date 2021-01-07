use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::messages::*;
use crate::util::read_be_u32;
use crate::PeerClient;

#[derive(Debug)]
pub struct Stream {
    peer_id: Vec<u8>,
    tcp_stream: TcpStream,
    am_choking: bool,
    am_interested: bool,
    peer_choking: bool,
    peer_interested: bool,
}

impl crate::PeerStreamT for Stream {}

impl crate::PeerStream for Stream {
    fn handshake(&mut self, info_hash: &[u8]) -> Result<(), crate::HandshakeError> {
        let handshake = Handshake {
            info_hash,
            peer_id: &self.peer_id,
        };
        let bytes: Vec<u8> = handshake.serialize();

        self.tcp_stream
            .write_all(&bytes)
            .map_err(|_e| crate::HandshakeError::TcpWrite)
            .and_then(move |_| {
                // handshake includes reading the return handshake
                let mut buf: [u8; 68] = [0; 68];

                match self.tcp_stream.read_exact(&mut buf) {
                    Ok(_) => {
                        let m = Handshake::new(&buf);
                        println!("handshake from peer: {:?}", m);
                        Ok(())
                    }
                    Err(_e) => Err(crate::HandshakeError::TcpReturnHandshakeRead(_e, buf)),
                }
            })
    }

    fn interested(&mut self) -> Result<(), crate::InterestedError> {
        self.am_interested = true;
        let bytes: Vec<u8> = Message::Interested.serialize();
        self.tcp_stream
            .write_all(&bytes)
            .map_err(|_e| crate::InterestedError::TcpWrite)
    }
}

pub struct PeerTcpClient {
    pub connections: Vec<Stream>,
    pub info_hash: Vec<u8>,
    message_sender: crate::client::MessageSender,
    message_receiver: crate::client::MessageReceiver,
}

impl PeerClient for PeerTcpClient {
    fn connect(peers: &[crate::tracker::Peer], info_hash: &[u8]) -> Self {
        let connections: Vec<Stream> = peers
            .iter()
            .filter_map(|p| {
                println!("connecting to peer {:?} over tcp", p);
                if let Ok(tcp_stream) =
                    TcpStream::connect_timeout(&p.socket_addr, std::time::Duration::from_secs(2))
                {
                    println!("connected to peer over tcp");
                    Some(Stream {
                        peer_id: p.id.clone(),
                        tcp_stream,
                        am_choking: true,
                        am_interested: false,
                        peer_choking: true,
                        peer_interested: false,
                    })
                } else {
                    println!("one of our peers didn't connect");
                    None
                }
            })
            .collect();

        let (sender, receiver) = channel();

        PeerTcpClient {
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

                let thread_handle = thread::spawn(move || {
                    loop {
                        let mut s = s_c.lock().unwrap().tcp_stream.try_clone().ok().unwrap(); // this ignores streams that failed to clone
                        let mut buf = [0u8; 4].to_vec();
                        match s
                            .read_exact(&mut buf)
                            .map_err(MessageParseError::PrefixLenRead)
                            .and_then(|_| {
                                let prefix_len = read_be_u32(&mut buf.as_slice())
                                    .map_err(|_| MessageParseError::PrefixLenConvert)?;

                                println!(
                                    "length of next message is {:?} from {:?}",
                                    prefix_len, buf
                                );

                                let mut message_buf = vec![0u8; prefix_len as usize];
                                s.read_exact(&mut message_buf)
                                    .map_err(|_| MessageParseError::MessageRead)
                                    .map(|_| (prefix_len, message_buf))
                            })
                            .and_then(|(prefix_len, message_buf)| {
                                tx.send((
                                    Arc::clone(&s_c) as Arc<Mutex<dyn crate::PeerStreamT>>,
                                    Message::new(Box::new(message_buf.into_iter()), prefix_len),
                                ))
                                .map_err(|_| MessageParseError::SendError)
                            }) {
                            Ok(_) => (),
                            Err(e) => panic!("a client broke down: {:?}", e),
                        }
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
