use std::fs::File;
use std::net::{SocketAddr, TcpStream};
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

mod logger;
use logger::Logger;

const TORRENT_FILE: &str = "charlie-chaplin-.-mabels-strange-predicament-1914-restored-short-silent-film-noir-comedy_archive.local.torrent";
const CONNECTION_TIMEOUT: Duration = Duration::from_millis(250);
const READ_TIMEOUT: Duration = Duration::from_millis(1000);
const PROGRESS_WAIT_TIME: Duration = Duration::from_secs(3);
const THREADS_PER_PEER: u8 = 1;
const MAX_IN_PROGRESS_REQUESTS_PER_CONNECTION: usize = 1;

type PeerThreads = Vec<JoinHandle<()>>;

#[derive(PartialEq, Debug)]
enum MessageResult {
    Ok,
    BadPeerHave,
    BadPeerPiece,
    BadPeerRequest,
}

struct TorrentProcessor {
    logger: Arc<RwLock<Logger>>,
    meta_info: MetaInfoFile,
    local_peer_id: String,
    torrent: Arc<RwLock<Torrent>>,
}

impl TorrentProcessor {
    fn new(torrent_file_path: &str, log_file_path: &str) -> Self {
        let meta_info = MetaInfoFile::from(File::open(torrent_file_path).unwrap());
        println!("meta info {:?}", meta_info);
        let local_peer_id = random_string();
        let logger = Arc::new(RwLock::new(Logger::new(log_file_path)));
        let torrent = Torrent::new(&meta_info);
        println!(
            "torrent num pieces {:?} num blocks {:?} len of pieces vec {:?}",
            torrent.total_pieces,
            torrent.total_blocks,
            torrent.pieces.len()
        );
        let torrent = Arc::new(RwLock::new(torrent));

        TorrentProcessor {
            logger,
            meta_info,
            local_peer_id,
            torrent,
        }
    }

    fn start(&self) {
        let info_encoded = percent_encode(&self.meta_info.info_hash, NON_ALPHANUMERIC).to_string();
        let possible_peers = Tracker::new()
            .track(
                &format!(
                    "{}?info_hash={}&peer_id={}",
                    &self.meta_info.announce, info_encoded, self.local_peer_id
                ),
                TrackerRequestParameters {
                    port: 8999,
                    uploaded: 0,
                    downloaded: 0,
                    left: 0,
                    event: Event::Started,
                },
            )
            .map(|resp: Vec<TrackerPeer>| {
                resp.into_iter()
                    .map(Peer::from)
                    // Don't connect to the client we are "pretending to be" at 127.0.0.1:8999
                    .filter(|x| match x.socket_addr {
                        std::net::SocketAddr::V4(sa) => {
                            !(*sa.ip() == std::net::Ipv4Addr::new(127, 0, 0, 1)
                                && sa.port() == 8999u16)
                        }
                        std::net::SocketAddr::V6(_) => true,
                    })
                    .map(|p| {
                        println!("peer {:?}, peer_id {:?}", p, std::str::from_utf8(&p.id));
                        p
                    })
                    .collect()
            });

        println!(
            "possible peers count {:?}",
            possible_peers
                .as_ref()
                .map(|pp: &Vec<Peer>| pp.len())
                .unwrap_or(0)
        );

        match possible_peers.map(|peers: Vec<Peer>| {
            let join_handles: Vec<PeerThreads> = peers
                .into_iter()
                .map(|p| self.generate_peer_threads(Arc::new(p)))
                .collect();
            join_handles
        }) {
            Ok(jhs) => {
                println!(
                    "total connections/threads working {:?}",
                    jhs.iter().flatten().count()
                );
                let t = Arc::clone(&self.torrent);
                spawn(move || loop {
                    sleep(PROGRESS_WAIT_TIME);
                    let t = t.read().unwrap();
                    println!("percent complete: {}", t.percent_complete);
                    println!("repeated completed blocks: {:?}", t.repeated_blocks);
                    println!("in progress blocks: {:?}", t.in_progress_blocks.len());
                });

                for jh in jhs {
                    for cjh in jh {
                        cjh.join().unwrap();
                    }
                }

                let files = match &self.meta_info.info {
                    Info::SingleFile {
                        piece_length: _,
                        pieces: _,
                        name: _,
                        file,
                    } => vec![file],
                    Info::MultiFile {
                        piece_length: _,
                        pieces: _,
                        directory_name: _,
                        files,
                    } => files.iter().collect(),
                };
                let write_res = self.torrent.read().unwrap().to_file(files);
                if write_res.iter().any(|r| r.is_err()) {
                    println!("write err when writing blocks to file {:?}", write_res)
                }
            }
            Err(e) => panic!("{:?}", e),
        }
    }

    fn generate_peer_threads(&self, peer: Arc<Peer>) -> PeerThreads {
        (0..THREADS_PER_PEER)
            .filter_map(|_| {
                let torrent = Arc::clone(&self.torrent);
                let peer = Arc::clone(&peer);
                let peer_addr = peer.socket_addr.to_string();
                let connection = self.connect(peer);
                let logger = Arc::clone(&self.logger);
                let work = move |mut connection: PeerConnection| {
                    let mut done = false;
                        while !done {
                            let message = connection.read_message();
                            match message {
                                Ok(message) => {
                                    let _ = logger.write().unwrap().log(&format!("From: {}, To (me): {}, Message: {}", connection.peer_addr, connection.local_addr, message));
                                    let result = process_message(Arc::clone(&torrent), message, &mut connection);
                                    if result != MessageResult::Ok {
                                        println!("got a err for message result which means some odd scenario occurred {:?}", result);
                                    }
                                }
                                Err(e) => {
                                    match e {
                                        MessageParseError::ConnectionRefused => {
                                            println!("Exiting {:?}", e);
                                            done = true;
                                            continue;
                                        },
                                        MessageParseError::ConnectionReset => {
                                            println!("Exiting {:?}", e);
                                            done = true;
                                            continue;
                                        },
                                        MessageParseError::ConnectionAborted => {
                                            println!("Exiting {:?}", e);
                                            done = true;
                                            continue;
                                        },
                                        MessageParseError::WouldBlock => {
                                            // println!("would block");
                                        },
                                        MessageParseError::TimedOut => {
                                        },
                                        me => {
                                            println!("Exiting {:?}", me);
                                            done = true;
                                            continue;
                                        },
                                    }
                                }
                            }
                            done = torrent.read().unwrap().are_we_done_yet();
                            if done {
                                println!("done because torrent said so");
                            }
                        }
                        println!("a connection has finally exited on its own... still being awaited by main potentially....");
                };
                match connection {
                    Ok(connection) => {
                        Some(spawn(move || work(connection)))
                    }
                    Err(e) => {
                        println!("connection err with client {:?}: {:?}", peer_addr, e);
                        None
                    }
                }
            })
            .collect::<Vec<JoinHandle<()>>>()
    }

    fn connect(&self, peer: Arc<Peer>) -> Result<PeerConnection, SendError> {
        let logger = self.logger.clone();
        let stream =
            TcpStream::connect_timeout(&peer.socket_addr, CONNECTION_TIMEOUT).map(|stream| {
                let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
                stream
            });
        stream.map_err(SendError::Connect).and_then(|s| {
            PeerConnection::new(
                Stream::Tcp(s),
                &self.meta_info.info_hash,
                self.local_peer_id.as_bytes(),
                &peer.id,
                Box::new(
                    move |message: (crate::Message, SocketAddr, SocketAddr),
                          original_bytes: &[u8]| {
                        let _ = logger.write().unwrap().log(&format!(
                            "From (me): {}, To: {}, Message: {}  ----  {:?}",
                            message.2, message.1, message.0, original_bytes
                        ));
                    },
                ),
            )
        })
    }
}

fn request_blocks(torrent: Arc<RwLock<Torrent>>, connection: &mut PeerConnection) {
    if !connection.is_choked {
        let in_progress = connection.in_progress_requests;
        let to_request = MAX_IN_PROGRESS_REQUESTS_PER_CONNECTION - in_progress;
        connection.in_progress_requests += to_request;
        let mut t = torrent.write().unwrap();
        let blocks: Vec<PieceIndexOffsetLength> = (0..to_request)
            .filter_map(|_| {
                let bf = connection.bitfield.as_ref().unwrap();
                t.get_next_block(bf)
            })
            .collect();
        for b in blocks {
            let message = Message::Request {
                index: b.0,
                begin: b.1,
                length: b.2,
            };
            connection.write_message(message).unwrap();
        }
    }
}

fn process_message(
    torrent: Arc<RwLock<crate::Torrent>>,
    message: Message,
    connection: &mut PeerConnection,
) -> MessageResult {
    match message {
        Message::KeepAlive => {
            connection.write_message(Message::KeepAlive).unwrap();
            MessageResult::Ok
        }
        Message::Choke => {
            connection.is_choked = true;
            MessageResult::Ok
        }
        Message::UnChoke => {
            connection.is_choked = false;
            request_blocks(torrent, connection);
            MessageResult::Ok
        }
        Message::Interested => MessageResult::Ok,
        Message::NotInterested => MessageResult::Ok,
        Message::Have { index } => {
            if index >= torrent.read().unwrap().total_pieces {
                MessageResult::BadPeerHave
            } else {
                if let Some(bf) = connection.bitfield.as_mut() {
                    bf.set(index as usize)
                }
                connection.is_local_interested = true;
                connection.write_message(Message::Interested).unwrap();
                MessageResult::Ok
            }
        }
        Message::BitField(bf) => {
            connection.is_local_interested = true;
            connection.bitfield = Some(bf.into());
            connection.write_message(Message::Interested).unwrap();
            MessageResult::Ok
        }
        Message::Request {
            index,
            begin: _begin,
            length: _length,
        } => {
            if index >= torrent.read().unwrap().total_pieces {
                MessageResult::BadPeerRequest
            } else {
                MessageResult::Ok
            }
        }
        Message::Piece {
            index,
            offset,
            data,
        } => {
            if !data.is_empty() {
                torrent.write().unwrap().fill_block((index, offset, &data));
                connection.in_progress_requests -= 1;
                request_blocks(torrent, connection);
                MessageResult::Ok
            } else {
                MessageResult::BadPeerPiece
            }
        }
    }
}

fn main() {
    // this program is just trying to connect to as many seeders as possible and go nuts downloading
    let tp = TorrentProcessor::new(TORRENT_FILE, "log.txt");
    tp.start();

    // Now, we also need to stick around and stay connected to the tracker long term so we can connect multiple clients for our own little localhost swarm for no reason except to learn

    // For now, though, can I write a client more easily in JS so I can just test that my client can successfully download?
}
