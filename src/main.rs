use std::fs::File;
use std::net::TcpStream;
use std::sync::{Arc, RwLock};
use std::thread::{sleep, spawn, JoinHandle};
use std::time::Duration;

use percent_encoding::{percent_encode, NON_ALPHANUMERIC};

mod bencode;
use bencode::*;

mod meta_info_file;
use meta_info_file::*;

mod tracker;
use tracker::{Event, Peer, Tracker, TrackerPeer, TrackerRequestParameters};

mod messages;
use messages::*;

mod util;
use util::random_string;

mod connection;
use connection::*;

mod torrent;
use torrent::*;

mod bitfield;
use bitfield::BitField;

const TORRENT_FILE: &str = "Charlie_Chaplin_Mabels_Strange_Predicament.avi.torrent";
const CONNECTION_TIMEOUT: Duration = Duration::from_millis(750);
const READ_TIMEOUT: Duration = Duration::from_millis(750);
const PROGRESS_WAIT_TIME: Duration = Duration::from_secs(15);
const THREADS_PER_PEER: u8 = 1;
const BLOCKS_PER_REQUEST: u8 = 5;
const INFO_HASH_BYTES: usize = 20;

fn connect(
    peer: Arc<Peer>,
    info_hash: [u8; INFO_HASH_BYTES],
    my_peer_id: String,
) -> Result<PeerConnection, SendError> {
    let stream = TcpStream::connect_timeout(&peer.socket_addr, CONNECTION_TIMEOUT).map(|stream| {
        let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
        stream
    });
    stream.map_err(SendError::Connect).and_then(|s| {
        PeerConnection::new(Stream::Tcp(s), &info_hash, my_peer_id.as_bytes(), &peer.id)
    })
}

type Blocks = Vec<Option<(u32, u32, u32)>>;

fn request_blocks(torrent: Arc<RwLock<Torrent>>, c: &mut PeerConnection) -> bool {
    if !c.is_choked {
        let bf = c.bitfield.as_ref().unwrap();
        let mut t = torrent.write().unwrap();
        let blocks: Blocks = (0..BLOCKS_PER_REQUEST)
            .map(|_| t.get_next_block(&bf))
            .collect();
        for b in blocks {
            match b {
                Some((index, offset, length)) => {
                    c.write_message(Message::Request {
                        index,
                        begin: offset,
                        length,
                    })
                    .unwrap();
                }
                None => return true,
            }
        }
    }
    false
}

fn are_we_done_yet(torrent: Arc<RwLock<Torrent>>) -> bool {
    let t = torrent.read().unwrap();
    t.are_we_done_yet()
}

fn process_frame(
    torrent: Arc<RwLock<crate::Torrent>>,
    frame: Message,
    c: &mut PeerConnection,
) -> bool {
    let t = Arc::clone(&torrent);
    match frame {
        Message::KeepAlive => {
            c.write_message(Message::KeepAlive).unwrap();
            request_blocks(torrent, c);
        }
        Message::Choke => {
            c.is_choked = true;
        }
        Message::UnChoke => {
            c.is_choked = false;
            request_blocks(torrent, c);
        }
        Message::Interested => (),
        Message::NotInterested => (),
        Message::Have { index } => {
            if index >= torrent.read().unwrap().total_pieces {
                // break fast; a crazy peer is among us
                return true;
            }
            let is_interested = c.is_local_interested;
            if !is_interested {
                c.is_local_interested = true;
                c.write_message(Message::Interested).unwrap();
            }
        }
        Message::BitField(bf) => {
            let is_interested = c.is_local_interested;
            if !is_interested {
                c.is_local_interested = true;
                c.bitfield = Some(bf.into());
                c.write_message(Message::Interested).unwrap();
            }
        }
        Message::Request {
            index,
            begin: _begin,
            length: _length,
        } => {
            if index >= torrent.read().unwrap().total_pieces {
                // break fast; a crazy peer is among us
                return true;
            }
        }
        Message::Piece {
            index,
            offset,
            data,
        } => {
            if !data.is_empty() {
                {
                    torrent.write().unwrap().fill_block((index, offset, &data));
                }
                request_blocks(torrent, c);
            } else {
                // break fast; a crazy peer is among us
                return true;
            }
        }
    };
    are_we_done_yet(t)
}

fn generate_peer_threads(
    p: Arc<Peer>,
    peer_id: String,
    info_hash: [u8; INFO_HASH_BYTES],
    t: Arc<RwLock<Torrent>>,
) -> PeerThreads {
    (0..THREADS_PER_PEER)
        .map(|_| {
            let my_peer_id = peer_id.clone();
            let t = Arc::clone(&t);
            let p = Arc::clone(&p);
            spawn(move || match connect(p, info_hash, my_peer_id) {
                Ok(mut c) => {
                    let mut done = false;
                    while !done {
                        done = are_we_done_yet(Arc::clone(&t));
                        let m = c.read_message();
                        match m {
                            Ok(frame) => {
                                done = process_frame(Arc::clone(&t), frame, &mut c);
                            }
                            Err(_) => {
                                continue;
                            }
                        }
                        request_blocks(Arc::clone(&t), &mut c);
                    }
                }
                Err(e) => println!("connection err: {:?}", e),
            })
        })
        .collect::<Vec<JoinHandle<()>>>()
}

type TrackerPeerResponse = Box<dyn Iterator<Item = TrackerPeer>>;
type PeerThreads = Vec<JoinHandle<()>>;

fn main() {
    let meta_info = MetaInfoFile::from(File::open(TORRENT_FILE).unwrap());
    let info_encoded = percent_encode(&meta_info.info_hash, NON_ALPHANUMERIC).to_string();
    let peer_id = random_string();

    let peers = Tracker::new()
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
        .map(|resp: TrackerPeerResponse| resp.map(Peer::from).collect());

    let torrent = Arc::new(RwLock::new(Torrent::new(&meta_info)));
    if let Ok((jhs, torrent)) = peers.map(|peers: Vec<Peer>| {
        let join_handles: Vec<PeerThreads> = peers
            .into_iter()
            .map(|p| {
                let peer_id = peer_id.clone();
                let t = Arc::clone(&torrent);
                generate_peer_threads(Arc::new(p), peer_id, meta_info.info_hash, t)
            })
            .collect();
        (join_handles, torrent)
    }) {
        let t = Arc::clone(&torrent);
        spawn(move || loop {
            sleep(PROGRESS_WAIT_TIME);
            let t = t.read().unwrap();
            println!("percent complete: {}", t.percent_complete);
            println!("repeated completed blocks: {:?}", t.repeated_blocks);
        });

        for jh in jhs {
            for cjh in jh {
                cjh.join().unwrap();
            }
        }

        let _ = torrent.read().unwrap().to_file();
    } else {
        panic!("{:?}",);
    }
}
