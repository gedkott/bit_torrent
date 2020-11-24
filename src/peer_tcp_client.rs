use std::io::{Read, Write};
use std::net::TcpStream;

const P_STR_LEN: u8 = 19;
const P_STR: &str = "BitTorrent protocol";

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

    fn choke(self) -> Self {
        self
    }

    fn unchoke(self) -> Self {
        self
    }

    fn interested(self) -> Self {
        self
    }

    fn not_interested(self) -> Self {
        self
    }

    fn have(self) -> Self {
        self
    }

    fn bitfield(self) -> Self {
        self
    }

    fn request(self) -> Self {
        self
    }

    fn piece(self) -> Self {
        self
    }

    fn cancel(self) -> Self {
        self
    }

    fn handshake(mut self, info_hash: &[u8]) -> Self {
        // do handshake
        println!("about to start writing handshake to wire");
        if let Err(e) = self
            .tcp_stream
            .write_all(&WireProtocolEncoding::HandshakeInteger(P_STR_LEN).encode())
            .and_then(|_| {
                self.tcp_stream
                    .write_all(&WireProtocolEncoding::String(P_STR).encode())
            })
            .and_then(|_| self.tcp_stream.write_all(&[0, 0, 0, 0, 0, 0, 0, 0]))
            .and_then(|_| self.tcp_stream.write_all(info_hash))
            .and_then(|_| self.tcp_stream.write_all(&self.peer_id))
        {
            println!("something didn't work out... {:?}", e);
        } else {
            println!("finished writing handshake to wire")
        }

        let mut buf = [0; 100];
        let mut attempts = 25;

        while attempts != 0 {
            let n = match self.tcp_stream.read(&mut buf) {
                Ok(n) => n,
                Err(e) => {
                    println!("got errr while reading: {}", e);
                    return self;
                }
            };
            // println!("got bytes potentially...");
            if n > 0 {
                println!("read bytes {:?}: {:?}", n, buf);
            } else {
                // 1. This reader has reached its "end of file" and will likely no longer be able to produce bytes. Note that this does not mean that the reader will always no longer be able to produce bytes.
                // 2. The buffer specified was 0 bytes in length.
                // println!("no bytes for 1 of 2 reasons");
            }
            attempts -= 1;
        }

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
                        // state: WireProtocolState::PreHandshake
                    })
                } else {
                    println!("one of our peers didn't get along with us");
                    None
                }
            })
            .map(|s: Stream| s.handshake(info_hash))
            // .map(|s: Stream| s.)
            .collect();
        PeerTcpClient { connections }
    }
}
