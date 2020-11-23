use crate::bencode;
use reqwest::blocking::Response;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

#[derive(PartialEq, Eq)]
pub enum Event {
    Started,
}

#[derive(Debug)]
pub struct Peer {
    pub socket_addr: SocketAddr,
    pub id: Vec<u8>,
}

#[derive(Debug)]
pub struct TrackerResponse {
    pub peers: Vec<TrackerPeer>,
}

#[derive(Debug)]
pub enum TrackerResponseError {
    BdecodeFailure(bencode::BencodeParseError),
    NoPeerKey,
    HTTPError(reqwest::Error),
    UnexpectedBencodable(bencode::Bencodable),
    MisalignedPeers,
    NoPeerByteString {
        original_string: bencode::Bencodable,
    },
}

pub struct TrackerRequestParameters {
    pub port: u16,
    pub uploaded: u32,
    pub downloaded: u32,
    pub left: u32,
    pub event: Event,
}

pub struct Tracker {
    client: reqwest::blocking::Client,
}

impl From<&bencode::BencodableByteString> for Result<Vec<SocketAddr>, TrackerResponseError> {
    fn from(b: &bencode::BencodableByteString) -> Result<Vec<SocketAddr>, TrackerResponseError> {
        let peer_bytes: &[u8] = b.as_bytes();
        let total_bytes = peer_bytes.len();
        if total_bytes % 6 == 0 {
            let mut socket_addrs: Vec<SocketAddr> = vec![];
            let mut i = 0;
            while i < total_bytes {
                let ip_bytes = &peer_bytes[i..i + 6];
                let ip = Ipv4Addr::new(ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3]);
                let port = u16::from_be_bytes([peer_bytes[4], peer_bytes[5]]);
                let socket_addr = SocketAddr::V4(SocketAddrV4::new(ip, port));
                socket_addrs.push(socket_addr);
                i += 6;
            }

            Ok(socket_addrs)
        } else {
            Err(TrackerResponseError::MisalignedPeers)
        }
    }
}

struct BencodableList<'a> {
    list: &'a [bencode::Bencodable],
}

impl<'a> From<BencodableList<'a>> for Result<Vec<TrackerPeer>, TrackerResponseError> {
    fn from(b: BencodableList) -> Result<Vec<TrackerPeer>, TrackerResponseError> {
        let mut rl = vec![];
        for b in b.list {
            match b {
                bencode::Bencodable::Dictionary(btm) => {
                    let port = btm
                        .get(&bencode::BencodableByteString::from("port"))
                        .ok_or_else(|| TrackerResponseError::UnexpectedBencodable(b.clone()))
                        .and_then(|port| match port {
                            bencode::Bencodable::Integer(i) => Ok(i),
                            _ => Err(TrackerResponseError::UnexpectedBencodable(b.clone())),
                        })
                        .unwrap();

                    let ip: std::net::Ipv4Addr = btm
                        .get(&bencode::BencodableByteString::from("ip"))
                        .ok_or_else(|| TrackerResponseError::UnexpectedBencodable(b.clone()))
                        .and_then(|ip| match ip {
                            bencode::Bencodable::ByteString(bs) => Ok(bs),
                            _ => Err(TrackerResponseError::UnexpectedBencodable(b.clone())),
                        })
                        .and_then(|s| {
                            s.as_string()
                                .map_err(|_| TrackerResponseError::UnexpectedBencodable(b.clone()))
                        })
                        .and_then(|s| {
                            s.parse::<std::net::Ipv4Addr>()
                                .map_err(|_| TrackerResponseError::UnexpectedBencodable(b.clone()))
                        })
                        .unwrap();

                    let peer_id = btm
                        .get(&bencode::BencodableByteString::from("peer id"))
                        .ok_or_else(|| TrackerResponseError::UnexpectedBencodable(b.clone()))
                        .and_then(|id| match id {
                            bencode::Bencodable::ByteString(bs) => Ok(bs.as_bytes().to_vec()),
                            _ => Err(TrackerResponseError::UnexpectedBencodable(b.clone())),
                        })
                        .unwrap();

                    rl.push(TrackerPeer::Peer(Peer {
                        socket_addr: SocketAddr::from((ip, *port as u16)),
                        id: peer_id,
                    }));
                }
                _ => return Err(TrackerResponseError::UnexpectedBencodable(b.clone())),
            }
        }
        Ok(rl)
    }
}

#[derive(Debug)]
pub enum TrackerPeer {
    Peer(Peer),
    SocketAddr(SocketAddr),
}

impl Tracker {
    pub fn new() -> Self {
        Tracker {
            client: reqwest::blocking::Client::new(),
        }
    }

    pub fn track(
        &self,
        announce_url: &str,
        trp: TrackerRequestParameters,
    ) -> Result<TrackerResponse, TrackerResponseError> {
        let request = self
            .client
            .get(announce_url)
            .query(&[(
                "event",
                match trp.event {
                    Event::Started => "started",
                },
            )])
            .query(&[("port", trp.port)])
            .query(&[("uploaded", trp.uploaded)])
            .query(&[("downloaded", trp.downloaded)])
            .query(&[("left", trp.left)])
            .build()
            .map_err(TrackerResponseError::HTTPError)?;

        self.client
            .execute(request)
            .map_err(TrackerResponseError::HTTPError)
            .and_then(|r: Response| {
                let bytes = r.bytes().map_err(TrackerResponseError::HTTPError)?;
                bencode::bdecode(&*bytes).map_err(TrackerResponseError::BdecodeFailure)
            })
            .and_then(|bencodable| match bencodable {
                bencode::Bencodable::Dictionary(mut btm) => {
                    let peers_bytes: Option<bencode::Bencodable> =
                        btm.remove(&bencode::BencodableByteString::from("peers"));
                    peers_bytes.ok_or(TrackerResponseError::NoPeerKey)
                }
                _ => Err(TrackerResponseError::UnexpectedBencodable(bencodable)),
            })
            .and_then(|peers| match peers {
                // A bytestring is one way to communicate a compact representation of peers
                bencode::Bencodable::ByteString(bs) => Result::from(&bs).map(|sas| {
                    sas.iter()
                        .map(|sa| TrackerPeer::SocketAddr(*sa))
                        .collect::<Vec<TrackerPeer>>()
                }),

                // alternatively, get a bencodable that is more structured as a List of Dictionaries containing keys IP, peer id, and port with values
                bencode::Bencodable::List(ld) => Result::from(BencodableList { list: &ld }),

                _ => Err(TrackerResponseError::NoPeerByteString {
                    original_string: peers,
                }),
            })
            .map(|peers| TrackerResponse { peers })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_correctly_converts_bytes_to_ip_addrs() {
        let example: &[u8] = &[
            0x49 as u8, 0x8C as u8, 0xCD as u8, 0x54 as u8, 0x23 as u8, 0x27 as u8, 0x49 as u8,
            0x8C as u8, 0xCD as u8, 0x54 as u8, 0x23 as u8, 0x27 as u8,
        ];
        assert_eq!(
            Result::from(&bencode::BencodableByteString::from(example)).unwrap(),
            vec![
                "73.140.205.84:8999"
                    .parse::<std::net::SocketAddr>()
                    .unwrap(),
                "73.140.205.84:8999"
                    .parse::<std::net::SocketAddr>()
                    .unwrap()
            ]
        );
    }
}
