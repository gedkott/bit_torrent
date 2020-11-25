use std::io::{Read, Write};
use std::net::TcpStream;

/**
 * 0000   50 3e aa 31 55 c1 9c 3d cf 9f 41 d2 08 00 45 20
0010   01 25 eb 4e 40 00 26 06 ba a0 c0 83 2c 87 c0 a8
0020   00 11 50 4b d0 76 3f f8 1f 02 f3 4f 77 57 80 18
0030   00 0b dd 2d 00 00 01 01 08 0a dd a3 e3 37 8b fd
0040   29 c0 - 13 42 69 74 54 6f 72 72 65 6e 74 20 70 72
0050   6f 74 6f 63 6f 6c 00 00 00 00 00 18 00 05 ae e0
0060   f0 08 2c c2 f4 49 41 2c 1d d8 af 4c 58 d9 aa ee
0070   4b 5c 2d 44 45 31 33 46 30 2d 36 28 36 63 34 5a
0080   61 57 4f 32 61 68 00 00 00 a4 05 ff ff ff ff ff
0090   ff ff ff ff ff ff ff ff ff ff ff ff ff ff ff ff
00a0   ff ff ff ff ff ff ff ff ff ff ff ff ff ff ff ff
00b0   ff ff ff ff ff ff ff ff ff ff ff ff ff ff ff ff
00c0   ff ff ff ff ff ff ff ff ff ff ff ff ff ff ff ff
00d0   ff ff ff ff ff ff ff ff ff ff ff ff ff ff ff ff
00e0   ff ff ff ff ff ff ff ff ff ff ff ff ff ff ff ff
00f0   ff ff ff ff ff ff ff ff ff ff ff ff ff ff ff ff
0100   ff ff ff ff ff ff ff ff ff ff ff ff ff ff ff ff
0110   ff ff ff ff ff ff ff ff ff ff ff ff ff ff ff ff
0120   ff ff ff ff ff ff ff ff ff ff ff ff ff ff 00 00
0130   00 01 01

 */

#[derive(Debug)]
struct Handshake<'a> {
    p_str_len: u8,
    p_str: &'a str,
    reserved_bytes: &'a [u8],
    info_hash: &'a [u8],
    peer_id: &'a [u8],
}

#[derive(Debug)]
enum HandshakeParseError {
    Generic,
}

fn parse_handshake(handshake_binary: &[u8]) -> Result<Handshake<'_>, HandshakeParseError> {
    let p_str_len = {
        *handshake_binary
            .get(0)
            .ok_or(HandshakeParseError::Generic)?
    };
    let len: usize = (1 + p_str_len).into();
    let p_str = {
        handshake_binary
            .get(1..len)
            .ok_or(HandshakeParseError::Generic)
            .and_then(|s| std::str::from_utf8(s).map_err(|_| HandshakeParseError::Generic))?
    };
    let reserved_bytes = {
        handshake_binary
            .get(len..len + 8)
            .ok_or(HandshakeParseError::Generic)?
    };
    let info_hash = {
        handshake_binary
            .get(len + 8..len + 8 + 20)
            .ok_or(HandshakeParseError::Generic)?
    };
    let peer_id = {
        handshake_binary
            .get(len + 8 + 20..len + 8 + 20 + 20)
            .ok_or(HandshakeParseError::Generic)?
    };

    Ok(Handshake {
        p_str_len,
        p_str,
        reserved_bytes,
        info_hash,
        peer_id,
    })
}

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

trait StreamI {
    fn should_download_block(&self) -> bool;

    fn should_upload_block(&self) -> bool;

    fn choke(self) -> Self;

    fn unchoke(self) -> Self;

    fn interested(self) -> Self;

    fn not_interested(self) -> Self;

    fn have(self) -> Self;

    fn bitfield(self) -> Self;

    fn request(self) -> Self;

    fn piece(self) -> Self;

    fn cancel(self) -> Self;
}

impl StreamI for Stream {
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
}

impl Stream {
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

        let mut buf = [0; 512];
        // let mut attempts = 25;

        // while attempts != 0 {
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
        // println!("got bytes potentially...");
        if n > 0 {
            println!("read bytes {:?}: {:?}", n, buf);

            let hand_shake = parse_handshake(&buf);
            println!("handshake {:?}", hand_shake);
        } else {
            // 1. This reader has reached its "end of file" and will likely no longer be able to produce bytes. Note that this does not mean that the reader will always no longer be able to produce bytes.
            // 2. The buffer specified was 0 bytes in length.
            println!("no bytes for 1 of 2 reasons");
        }
        // attempts -= 1;
        // }

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
