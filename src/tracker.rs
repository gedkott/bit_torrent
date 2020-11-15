use crate::bencode;
use reqwest::blocking::Response;

pub struct ClientPort(pub u16);
pub struct TotalBytes(pub u32);

#[derive(PartialEq, Eq)]
pub enum Event {
    Started,
}

#[derive(Debug)]
pub struct TrackerResponse {
    peers: Vec<std::net::SocketAddr>,
}

#[derive(Debug)]
pub enum TrackerResponseError {
    BdecodeFailure(bencode::BencodeParseError),
    NoPeerKey,
    HTTPError(reqwest::Error),
    UnexpectedBencodable(bencode::Bencodable),
    MisalignedPeers,
    NoPeerByteString,
}

pub struct TrackerRequestParameters {
    pub port: ClientPort,
    pub uploaded: TotalBytes,
    pub downloaded: TotalBytes,
    pub left: TotalBytes,
    pub event: Event,
}

pub struct Tracker {
    client: reqwest::blocking::Client
}

impl Tracker {
    pub fn new() -> Self {
        Tracker {
            client: reqwest::blocking::Client::new()
        }
    }

    pub fn track(
        &self,
        announce_url: &str,
        trp: TrackerRequestParameters,
    ) -> Result<TrackerResponse, TrackerResponseError> {
        let request = self.client
            .get(announce_url)
            .query(&[
                (
                    "event", 
                    match trp.event {
                        Event::Started => "started",
                    }
                )
            ])
            .query(&[("port", trp.port.0)])
            .query(&[("uploaded", trp.uploaded.0)])
            .query(&[("downloaded", trp.downloaded.0)])
            .query(&[("left", trp.left.0)])
            .build()
            .map_err(TrackerResponseError::HTTPError)?;

        self.client
            .execute(request)
            .map_err(TrackerResponseError::HTTPError)
            .and_then(|r: Response| {
                let bytes = r.bytes().map_err(TrackerResponseError::HTTPError)?;
                bencode::bdecode(&*bytes)
                    .map_err(TrackerResponseError::BdecodeFailure)
            })
            .and_then(|bencodable| match bencodable {
                bencode::Bencodable::Dictionary(mut btm) => {
                    let peers_bytes: Option<bencode::Bencodable> = btm.remove(&bencode::BencodableByteString::from("peers"));
                    peers_bytes
                        .ok_or(TrackerResponseError::NoPeerKey)
                }
                _ => Err(TrackerResponseError::UnexpectedBencodable(bencodable)),
            })
            .and_then(|peers| {
                match peers {
                    bencode::Bencodable::ByteString(bs) => {
                        let peer_bytes = bs.0;
                        let total_bytes = peer_bytes.len();
                        if total_bytes % 6 == 0 {
                            let mut socket_addrs: Vec<std::net::SocketAddr> = vec![];
                            let mut i = 0;
                            while i < total_bytes {
                                let ip_bytes = &peer_bytes[i..i+6];
                                let ip = std::net::Ipv4Addr::new(
                                    ip_bytes[0],
                                    ip_bytes[1],
                                    ip_bytes[2],
                                    ip_bytes[3],
                                );
                                let port = u16::from_be_bytes([peer_bytes[4], peer_bytes[5]]);
                                let socket_addr = std::net::SocketAddr::V4(std::net::SocketAddrV4::new(
                                    ip, port,
                                ));
                                socket_addrs.push(socket_addr);
                                i = i + 6;
                            }

                            Ok(TrackerResponse {
                                peers: socket_addrs,
                            })
                        } else {
                            Err(TrackerResponseError::MisalignedPeers)
                        }
                    }
                    _ => Err(TrackerResponseError::NoPeerByteString),
                }
            })
    }
}
