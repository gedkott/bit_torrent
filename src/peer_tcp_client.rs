use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc::{channel, Receiver};
use std::thread;

use crate::messages::*;

#[derive(Debug)]
pub struct Stream {
    peer_id: Vec<u8>,
    tcp_stream: TcpStream,
    am_choking: bool,
    am_interested: bool,
    peer_choking: bool,
    peer_interested: bool,
}

impl Stream {
    // fn _should_download_block(&self) -> bool {
    //     self.am_interested && !self.peer_choking
    // }

    // fn _should_upload_block(&self) -> bool {
    //     !self.am_choking && self.peer_interested
    // }

    fn _have(self) -> Self {
        self
    }

    fn _bitfield(self) -> Self {
        self
    }

    fn _request(self) -> Self {
        self
    }

    fn _piece(self) -> Self {
        self
    }

    fn _cancel(self) -> Self {
        self
    }
}

impl Stream {
    pub fn handshake(&mut self, info_hash: &[u8]) {
        let handshake = Handshake {
            info_hash,
            peer_id: &self.peer_id,
        };
        println!("handshake to peer: {:?}", handshake);

        let bytes: Vec<u8> = handshake.serialize();

        if let Err(_e) = self.tcp_stream.write_all(&bytes) {
        } else {
        }

        // handshake includes reading the return handshake
        let mut buf: [u8; 68] = [0; 68];

        let _n = match self.tcp_stream.read(&mut buf) {
            Ok(n) => {
                let m = Handshake::new(&buf);
                println!("handshake from peer: {:?}", m);
                n
            }
            Err(_e) => {
                return;
            }
        };
    }

    pub fn interested(&mut self) {
        let bytes: Vec<u8> = Message::Interested.serialize();

        println!("identifying as intereested in peer: {:?}", self);

        if let Err(_e) = self.tcp_stream.write_all(&bytes) {
        } else {
        }
    }

    pub fn unchoke_self(&mut self) {
        self.am_choking = false;
        println!("stream being unchoked by peer: {:?}", self);
    }

    pub fn choke_self(&mut self) {
        self.am_choking = true;
        println!("stream being choked by peer: {:?}", self);
    }
}

pub struct PeerTcpClient {
    pub connections: Vec<Stream>,
    pub info_hash: Vec<u8>,
}

use std::sync::{Arc, Mutex};

type R = Receiver<(Arc<Mutex<Stream>>, Result<Message, MessageParseError>)>;
pub struct Readers {
    pub receiver: R,
    pub threads: Vec<(std::thread::JoinHandle<()>, Arc<Mutex<Stream>>)>,
}

impl PeerTcpClient {
    pub fn connect(peers: &[crate::tracker::Peer], info_hash: &[u8]) -> Self {
        let connections: Vec<Stream> = peers
            .iter()
            .filter_map(|p| {
                println!("connecting to peer {:?} over tcp", p);
                if let Ok(tcp_stream) =
                    TcpStream::connect_timeout(&p.socket_addr, std::time::Duration::from_secs(3))
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
                    println!("one of our peers didn't get along with us");
                    None
                }
            })
            .collect();
        PeerTcpClient {
            connections,
            info_hash: info_hash.to_vec(),
        }
    }

    pub fn listen(self) -> Readers {
        let (sender, receiver) = channel();
        let threads = self
            .connections
            .into_iter()
            .map(|c| {
                let s_c = Arc::new(Mutex::new(c));
                let r_s_c = Arc::clone(&s_c);
                let tx = sender.clone();

                let thread_handle = thread::spawn(move || {
                    let mut s = s_c.lock().unwrap().tcp_stream.try_clone().ok().unwrap(); // this ignores streams that failed to clone
                    let mut buf = [0u8; 256].to_vec();
                    while let Ok(n) = s.read(&mut buf) {
                        if n > 0 {
                            let buf_iter = buf.clone().into_iter();
                            let m = Message::new(Box::new(buf_iter));
                            tx.send((Arc::clone(&s_c), m)).unwrap();
                        } else {
                        }
                    }
                });
                (thread_handle, r_s_c)
            })
            .collect();
        Readers { receiver, threads }
    }
}
