use std::convert::TryInto;
use std::io::{Read, Write};
use std::net::TcpStream;

const P_STR_LEN: u8 = 19;
const P_STR: &str = "BitTorrent protocol";
const RESERVED_BYTES: [u8; 8] = [0; 8];

#[derive(Debug)]
struct Handshake<'a> {
    info_hash: &'a [u8],
    peer_id: &'a [u8],
}

impl<'a> Handshake<'a> {
    fn serialize(&self) -> Vec<u8> {
        [
            WireProtocolEncoding::HandshakeInteger(P_STR_LEN).encode(),
            WireProtocolEncoding::String(P_STR).encode(),
            RESERVED_BYTES.to_vec(),
            self.info_hash.to_vec(),
            self.peer_id.to_vec(),
        ]
        .iter()
        .flatten()
        .cloned()
        .collect()
    }
}

#[derive(Debug)]
enum HandshakeParseError {
    PStrLen,
    PStr,
    ReservedBytes,
    InfoHash,
    PeerId,
}

fn parse_handshake(handshake_binary: &[u8]) -> Result<Handshake<'_>, HandshakeParseError> {
    let p_str_len: usize = (*handshake_binary
        .get(0)
        .ok_or(HandshakeParseError::PStrLen)?)
    .try_into()
    .map_err(|_| HandshakeParseError::PStrLen)?;

    let len: usize = 1 + p_str_len;

    let _p_str = handshake_binary
        .get(1..len)
        .ok_or(HandshakeParseError::PStr)
        .and_then(|s| std::str::from_utf8(s).map_err(|_| HandshakeParseError::PStr))?;

    let _reserved_bytes = handshake_binary
        .get(len..len + 8)
        .ok_or(HandshakeParseError::ReservedBytes)?;

    let info_hash = handshake_binary
        .get(len + 8..len + 8 + 20)
        .ok_or(HandshakeParseError::InfoHash)?;

    let peer_id = handshake_binary
        .get(len + 8 + 20..len + 8 + 20 + 20)
        .ok_or(HandshakeParseError::PeerId)?;

    Ok(Handshake { info_hash, peer_id })
}

struct Stream {
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
    fn handshake(mut self, info_hash: &[u8]) -> Self {
        // do handshake
        println!("about to start writing handshake to wire");

        let handshake = Handshake {
            info_hash,
            peer_id: &self.peer_id,
        };

        let bytes: Vec<u8> = handshake.serialize();

        if let Err(e) = self.tcp_stream.write_all(&bytes) {
            println!("something didn't work out... {:?}", e);
        } else {
            println!("finished writing handshake to wire {:?}", handshake)
        }

        let mut buf = [0; 512];

        let n = match self.tcp_stream.read(&mut buf) {
            Ok(n) => {
                println!("read {} bytes", n);
                n
            }
            Err(e) => {
                println!("got errr while reading: {}", e);
                return self;
            }
        };

        if n > 0 {
            let hand_shake = parse_handshake(&buf);
            println!("handshake {:?}", hand_shake);
            println!("peer id: {}", std::str::from_utf8(hand_shake.unwrap().peer_id).unwrap());
        } else {
            println!("no bytes for 1 of 2 reasons");
        }

        Stream {
            peer_id: self.peer_id,
            tcp_stream: self.tcp_stream,
            am_choking: self.am_choking,
            am_interested: self.am_interested,
            peer_choking: self.peer_choking,
            peer_interested: self.peer_interested,
        }
    }
}

enum WireProtocolEncoding<'a> {
    HandshakeInteger(u8),
    // PostHandshakeInteger(u32),
    String(&'a str),
}

impl WireProtocolEncoding<'_> {
    pub fn encode(&self) -> Vec<u8> {
        match self {
            WireProtocolEncoding::HandshakeInteger(i) => u8::to_be_bytes(*i).to_vec(),
            // WireProtocolEncoding::PostHandshakeInteger(i) => u32::to_be_bytes(*i).to_vec(),
            WireProtocolEncoding::String(s) => s.as_bytes().to_vec(),
        }
    }
}

pub struct PeerTcpClient {
    _connections: Vec<Stream>,
}

impl<'a> PeerTcpClient {
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
            .map(|s: Stream| s.handshake(info_hash))
            .collect();
        PeerTcpClient {
            _connections: connections,
        }
    }
}
