use std::fs::File;
use std::net::{SocketAddr, TcpStream};
use std::sync::{Arc, Mutex};
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
const PROGRESS_TIMER: Duration = Duration::from_secs(15);

fn connect(
    socket_addr: SocketAddr,
    info_hash: [u8; 20],
    peer_id: String,
) -> Result<PeerConnection, SendError> {
    TcpStream::connect_timeout(&socket_addr, CONNECTION_TIMEOUT)
        .map_err(SendError::Connect)
        .and_then(|s| PeerConnection::new(Stream::Tcp(s), &info_hash, peer_id.as_bytes()))
}

type Blocks = Vec<Option<(u32, u32, u32)>>;

fn request_blocks(torrent: Arc<Mutex<Torrent>>, c: &mut PeerConnection) -> bool {
    let bf = c.bitfield.as_ref().unwrap();
    let mut t = torrent.lock().unwrap();
    let blocks: Blocks = (0..5).map(|_| t.get_next_block(&bf)).collect();
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
    false
}

fn are_we_done_yet(torrent: Arc<Mutex<Torrent>>) -> bool {
    let t = torrent.lock().unwrap();
    t.are_we_done_yet()
}

fn process_frame(
    torrent: Arc<Mutex<crate::Torrent>>,
    frame: Message,
    c: &mut PeerConnection,
) -> bool {
    let t = Arc::clone(&torrent);
    match frame {
        Message::KeepAlive => {
            c.write_message(Message::KeepAlive).unwrap();
            request_blocks(torrent, c);
        }
        Message::Choke => (),
        Message::UnChoke => {
            request_blocks(torrent, c);
        }
        Message::Interested => (),
        Message::NotInterested => (),
        Message::Have { index: _index } => {
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
            index: _index,
            begin: _begin,
            length: _length,
        } => (),
        Message::Piece {
            index,
            offset,
            data,
        } => {
            {
                torrent.lock().unwrap().fill_block((index, offset, &data));
            }
            request_blocks(torrent, c);
        }
    };
    are_we_done_yet(t)
}

fn generate_peer_threads(
    p: Peer,
    peer_id: String,
    info_hash: [u8; 20],
    t: Arc<Mutex<Torrent>>,
) -> PeerThreads {
    (0..8)
        .map(|_| {
            let peer_id = peer_id.clone();
            let socket_addr = p.socket_addr;
            let t = Arc::clone(&t);
            spawn(move || {
                if let Ok(mut c) = connect(socket_addr, info_hash, peer_id) {
                    let mut done = false;
                    while !done {
                        let m = { c.read_message() };
                        match m {
                            Ok(frame) => {
                                done = process_frame(Arc::clone(&t), frame, &mut c);
                                if !done {
                                    request_blocks(Arc::clone(&t), &mut c);
                                }
                            }
                            Err(_) => {
                                break;
                            }
                        }
                    }
                }
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

    let torrent = Arc::new(Mutex::new(Torrent::new(&meta_info)));
    if let Ok((jhs, torrent)) = peers.map(|peers: Vec<Peer>| {
        let join_handles: Vec<PeerThreads> = peers
            .into_iter()
            .map(|p| {
                let peer_id = peer_id.clone();
                let t = Arc::clone(&torrent);
                generate_peer_threads(p, peer_id, meta_info.info_hash, t)
            })
            .collect();
        (join_handles, torrent)
    }) {
        let t = Arc::clone(&torrent);
        spawn(move || loop {
            sleep(PROGRESS_TIMER);
            let p = { t.lock().unwrap().progress() };
            println!(
                "progress: {}, completed: {}, in progress: {}, not requested: {}",
                p.0, p.1, p.2, p.3
            );
        });

        for jh in jhs {
            for cjh in jh {
                cjh.join().unwrap();
            }
        }

        let _ = torrent.lock().unwrap().to_file();
    } else {
        panic!("{:?}",);
    }
}
