use std::fs::File;
use std::io::prelude::*;
use std::sync::{Arc, Mutex};

use sha1::Sha1;

mod bencode;
use bencode::*;

mod meta_info_file;
use meta_info_file::*;

mod tracker;
use tracker::*;

mod messages;
use messages::*;

mod util;
use util::{random_string, AtomicCounter};

mod connection;
use connection::*;

const TORRENT_FILE: &str = "Charlie_Chaplin_Mabels_Strange_Predicament.avi.torrent";
const CONNECTION_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(100);

#[derive(Debug)]
struct Stats {
    from_tracker: AtomicCounter,
    tcp_connected: AtomicCounter,
    tcp_peers: AtomicCounter,
}

fn read_meta_info_file() -> (MetaInfoFile, String, [u8; 20]) {
    let mut f = File::open(TORRENT_FILE).unwrap();
    let mut bytes = Vec::new();
    f.read_to_end(&mut bytes).unwrap();
    let bytes_slice = bytes.as_slice();
    let bencodable = bdecode(bytes_slice).unwrap();
    let meta_info = MetaInfoFile::from(&bencodable);

    let info = match &bencodable {
        Bencodable::Dictionary(btm) => {
            let info_key = &BencodableByteString::from("info");
            match &btm[info_key] {
                Bencodable::Dictionary(btm) => bencode(&Bencodable::Dictionary(btm.clone())),
                _ => panic!("did not find info for info hash"),
            }
        }
        _ => panic!("did not find dictionary for Metainfo file structure for info hash"),
    };

    let info_hash = {
        let mut hasher = Sha1::new();
        hasher.update(&info.unwrap());
        hasher.digest().bytes()
    };

    let info_encoded =
        percent_encoding::percent_encode(&info_hash, percent_encoding::NON_ALPHANUMERIC)
            .to_string();

    (meta_info, info_encoded, info_hash)
}

fn process_frame(frame: Message, c: &mut PeerConnection) {
    match frame {
        Message::KeepAlive => {
            c.write_message(Message::KeepAlive);
        },
        Message::Choke => (),
        Message::UnChoke => {
            c.write_message(Message::Request {
                index: 0,
                begin: 0,
                length: 16384,
            });
        },
        Message::Interested => (),
        Message::NotInterested => (),
        Message::Have { index } => {
            c.write_message(Message::Interested);
        }
        Message::BitField(bf) => {
            c.write_message(Message::Interested);
        }
        Message::Request {
            index,
            begin,
            length,
        } => {
            // c.write_message(Message::Interested);
        }
        Message::Piece {
            index,
            offset,
            data,
        } => {
            println!(
                "got piece from index {:?} at offset {:?} for {:?} bytes",
                index,
                offset,
                data.len()
            )
        }
    }
}

fn main() {
    let (meta_info, info_encoded, info_hash) = read_meta_info_file();
    println!("torrent has {:?} pieces", meta_info.pieces().len());
    println!(
        "torrent pieces are each {:?} bytes",
        meta_info.piece_length()
    );
    println!("torrent file size is {:?} bytes", meta_info.file_length());
    let peer_id = Arc::new(random_string());
    let stats = Arc::new(Stats {
        from_tracker: AtomicCounter::new(),
        tcp_connected: AtomicCounter::new(),
        tcp_peers: AtomicCounter::new(),
    });

    if let Some(e) = Tracker::new()
        .track(
            &format!(
                "{}?info_hash={}&peer_id={}",
                &meta_info.announce, info_encoded, peer_id
            ),
            TrackerRequestParameters {
                port: 8999,
                uploaded: 0,
                downloaded: 0,
                left: 0,
                event: Event::Started,
            },
        )
        .map(|resp: Box<dyn Iterator<Item = tracker::TrackerPeer>>| {
            let stats = Arc::clone(&stats);
            resp.map(|tp| {
                stats.from_tracker.update();
                match tp {
                    TrackerPeer::Peer(p) => p,
                    TrackerPeer::SocketAddr(sa) => {
                        println!(
                            "weird peer from tracker with only socket addr, no ID: {:?}",
                            sa
                        );
                        let id = random_string();
                        tracker::Peer {
                            id: id.as_bytes().to_vec(),
                            socket_addr: sa,
                        }
                    }
                }
            })
            .collect()
        })
        .map(|peers: Vec<tracker::Peer>| {
            // in parallel, complete handshake sequence with each peer we connected to successfully
            let stats = Arc::clone(&stats);
            println!("kicking off handshaking for everyone");
            let peer_thread = move |p: tracker::Peer| {
                let socket_addr = p.socket_addr;
                let stats = Arc::clone(&stats);
                let peer_id = Arc::clone(&peer_id);
                let stats = Arc::clone(&stats);
                std::thread::spawn(move || {
                    if let Err(e) = std::net::TcpStream::connect_timeout(
                        &socket_addr,
                        CONNECTION_TIMEOUT,
                    )
                        .map_err(SendError::Connect)
                        .and_then(|s| {
                            stats.tcp_connected.update();
                            stats.tcp_peers.update();
                            PeerConnection::new(
                                Stream::Tcp(s),
                                &info_hash,
                                peer_id.as_bytes(),
                            )
                        })
                        .map(|mut c| {
                            // std::thread::spawn(move || {
                                loop {
                                    let r = c.read_message();
                                    match r {
                                        Ok(frame) => {
                                            println!("frame: {:?}", match &frame {
                                                Message::Piece { index, offset, .. } => format!("Piece {{ index: {:?}, offset: {:?} }}", index, offset),
                                                frame => format!("{:?}", frame) 
                                            });
                                            process_frame(frame, &mut c);
                                        },
                                        Err(e) => {
                                            panic!("could not read frame {:?}", e)
                                        }
                                    }
                                }
                            // });
                        }) {
                            println!("thread spawn went wonky {:?}", e);
                        }
                })
            };
            peers.into_iter().map(peer_thread)
        })
        .map(|jhs| {
            let stats = Arc::clone(&stats);
            std::thread::spawn(move || loop {
                println!("stats: {:?}", stats);
                std::thread::sleep(std::time::Duration::from_secs(10))
            });
            for jh in jhs {
                jh.join();
            }
        })
        .err()
    {
        println!("Error from tracking: {:#?}", e);
    }
}
