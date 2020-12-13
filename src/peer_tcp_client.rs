use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc::{channel, Receiver};
use std::thread;

use crate::messages::*;

pub struct Stream {
    peer_id: Vec<u8>,
    tcp_stream: TcpStream,
    am_choking: bool,
    am_interested: bool,
    peer_choking: bool,
    peer_interested: bool,
}

impl Stream {
    fn _should_download_block(&self) -> bool {
        self.am_interested && !self.peer_choking
    }

    fn _should_upload_block(&self) -> bool {
        !self.am_choking && self.peer_interested
    }

    fn _choke(self) -> Self {
        self
    }

    fn _unchoke(self) -> Self {
        self
    }

    fn _interested(self) -> Self {
        self
    }

    fn _not_interested(self) -> Self {
        self
    }

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
    pub fn handshake(&mut self, info_hash: &[u8]) -> () {
        let handshake = Handshake {
            info_hash,
            peer_id: &self.peer_id,
        };

        let bytes: Vec<u8> = handshake.serialize();

        println!("message {:?}", handshake);
        if let Err(e) = self.tcp_stream.write_all(&bytes) {
        } else {
        }

        let mut buf = [0; 68];

        let n = match self.tcp_stream.read(&mut buf) {
            Ok(n) => {
                n
            }
            Err(e) => {
                return ();
            }
        };

        // if n > 0 {
        //     let hand_shake = Handshake::new(&buf);
        // } else {
        // }

        ()
    }
}

pub struct PeerTcpClient {
    pub connections: Vec<Stream>,
    pub info_hash: Vec<u8>
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
            .map(|mut s: Stream| {
                s.handshake(info_hash); 
                s 
            } )
            .collect();
        PeerTcpClient {
            connections: connections,
            info_hash: info_hash.to_vec()
        }
    }

    pub fn listen(self) -> 
        (Receiver<Result<Message, MessageParseError>>, Vec<(std::thread::JoinHandle<()>, Stream)>) {
        let (sender, receiver) = channel();
        // let (outbound_sender, outbound_receiver) = channel();
        let threads = self.connections
            .into_iter()
            .filter_map(|c| {
                let mut s = c.tcp_stream.try_clone().ok()?; // this ignores streams that failed to clone
                let tx = sender.clone();
                let thread_handle = thread::spawn(move || {
                    let mut buf = [0u8; 256].to_vec();
                    while let Ok(n) = s.read(&mut buf) {
                        if n > 0 {
                            let buf_iter = buf.clone().into_iter();
                            let m = Message::new(Box::new(buf_iter));
                            tx.send(m).unwrap();
                        } else {

                        }
                    };
                });
                Some((thread_handle, c))
            })
            .collect();
        (receiver, threads)
    }

    pub fn write(self) {

    }
}
