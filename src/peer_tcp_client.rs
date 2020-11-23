use std::io::{Read, Write};
use std::net::{TcpStream};

use crate::TrackerPeer;

const P_STR_LEN: u8 = 19;
const P_STR: &str = "BitTorrent Protocol";

struct Stream {
    peer_id: Vec<u8>,
    tcp_stream: TcpStream,
    am_choking: bool,
    am_interested: bool,
    peer_choking: bool,
    peer_interested: bool,
}

impl Stream {
    fn should_download_block(&self) -> bool {
        self.am_interested && !self.peer_choking
    }

    fn should_upload_block(&self) -> bool {
        !self.am_choking && self.peer_interested
    }

    fn handshake(mut self, info_hash: &[u8]) -> Self {
        // do handshake
        println!("about to start writing handshake to wire");
        if let Err(e) = self
            .tcp_stream
            .write_all(
                WireProtocolEncoding::HandshakeInteger(P_STR_LEN)
                    .encode()
                    .as_ref(),
            )
            .and_then(|_| {
                self.tcp_stream
                    .write_all(WireProtocolEncoding::String(P_STR).encode().as_ref())
            })
            .and_then(|_| self.tcp_stream.write_all(&[0, 0, 0, 0, 0, 0, 0, 0]))
            .and_then(|_| self.tcp_stream.write_all(info_hash))
            .and_then(|_| self.tcp_stream.write_all(&self.peer_id))
        {
            println!("something didn't work out... {:?}", e);
        } else {
            println!("finished writing handshake to wire")
        }

        let mut buf = vec![];
        self.tcp_stream
            .read_to_end(&mut buf)
            .expect("could not read from tcp stream after attempting handshake");
        println!("read bytes: {:?}", buf);

        Stream {
            peer_id: self.peer_id,
            tcp_stream: self.tcp_stream,
            am_choking: self.am_choking,
            am_interested: self.am_interested,
            peer_choking: self.peer_choking,
            peer_interested: self.peer_interested,
            // state: WireProtocolState::Handshake,
        }
    }
}

enum WireProtocolEncoding<'a> {
    HandshakeInteger(u8),
    PostHandshakeInteger(u32),
    String(&'a str),
}

impl WireProtocolEncoding<'_> {
    pub fn encode(&self) -> Vec<u8> {
        match self {
            WireProtocolEncoding::HandshakeInteger(i) => u8::to_be_bytes(*i).to_vec(),
            WireProtocolEncoding::PostHandshakeInteger(i) => u32::to_be_bytes(*i).to_vec(),
            WireProtocolEncoding::String(s) => s.as_bytes().to_vec(),
        }
    }
}

// enum WireProtocolState {
//     PreHandshake,
//     Handshake
// }

pub struct PeerTcpClient {
    connections: Vec<Stream>,
}

impl<'a> PeerTcpClient {
    pub fn connect(peers: &[&crate::tracker::Peer], info_hash: &[u8]) -> Self {
        let connections: Vec<Stream> = peers
            .iter()
            .filter_map(|p| {
                println!("connecting to peer {:?} over tcp", p);
                if let Ok(tcp_stream) = TcpStream::connect_timeout(&p.socket_addr, std::time::Duration::from_secs(3)) {
                    println!("connected to peer over tcp");
                    Some(Stream {
                        peer_id: p.id.clone(),
                        tcp_stream,
                        am_choking: true,
                        am_interested: false,
                        peer_choking: true,
                        peer_interested: false,
                        // state: WireProtocolState::PreHandshake
                    })
                } else {
                    println!("one of our peers didn't get along with us");
                    None
                }
            })
            .map(|s: Stream| {
                s.handshake(info_hash)
            })
            .collect();
        PeerTcpClient { connections }
    }
}
