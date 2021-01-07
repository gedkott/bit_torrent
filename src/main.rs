use std::fs::File;
use std::io::prelude::*;
use std::sync::{Arc, Mutex};
use std::thread;

use sha1::Sha1;

mod bencode;
use bencode::*;

mod meta_info_file;
use meta_info_file::*;

mod tracker;
use tracker::*;

mod messages;

mod peer_tcp_client;
use peer_tcp_client::*;

mod peer_utp_client;

mod util;
use util::random_string;

mod client;
use client::*;

const TORRENT_FILE: &str = "Charlie_Chaplin_Mabels_Strange_Predicament.avi.torrent";

fn main() {
    let mut f = File::open(TORRENT_FILE).unwrap();
    let mut bytes = Vec::new();
    f.read_to_end(&mut bytes).unwrap();
    let bytes_slice = bytes.as_slice();
    let bencodable = bdecode(bytes_slice).unwrap();
    let meta_info = MetaInfoFile::from(&bencodable);
    let peer_id = random_string();

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
            println!("got peers, starting bit torrent protocol with them...");
            let tcp_peers_w_peer_id: Vec<tracker::Peer> = resp
                // TODO(): this is a random cap on the number of peers from the
                // tracker that we will attempt to establish a connection with
                .take(7)
                .map(|tp| match tp {
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
                })
                .collect::<Vec<tracker::Peer>>();
            Box::new(<PeerTcpClient as PeerClient>::connect(
                &tcp_peers_w_peer_id,
                &info_hash,
            )) as Box<PeerTcpClient>
        })
        .map(|ptc| {
            let r = ptc.listen();
            let message_receiver = r.receiver;

            type Threads = Vec<std::thread::JoinHandle<()>>;
            type Streams = Vec<Arc<Mutex<dyn PeerStreamT>>>;

            let (threads, streams): (Threads, Streams) =
                r.threads
                    .into_iter()
                    .fold((vec![], vec![]), |(mut ts, mut ss), (t, s)| {
                        ts.push(t);
                        ss.push(s);
                        (ts, ss)
                    });

            // in serial, handshake with each peer we connected to succesffully, processing the return handshake as well
            for stream in streams {
                match stream.lock().unwrap().handshake(&info_hash) {
                    Ok(_) => {}
                    Err(e) => {
                        println!("err handshaking: {:?}", e)
                    }
                }
            }

            // Now, with handshakes completed, begin waiting for messages from reader threads in a loop
            thread::spawn(move || {
                loop {
                    println!("waiting for message...");
                    let (_, message) = message_receiver.recv().unwrap();
                    if let Err(e) = message.map(|message| {
                        println!("message: {:?}", message);
                    }) {
                        println!("error message: {:?}", e);
                    } else {
                        // normal behavior
                    }
                }
            });

            for t in threads {
                let printable_thread = format!("{:?}", t.thread());
                match t.join() {
                    Ok(_) => println!("thread {:?} exited normally", printable_thread),
                    Err(e) => println!(
                        "thread {:?} exited abnormally with err {:?}",
                        printable_thread, e
                    ),
                }
            }
        })
        .err()
    {
        println!("Error from tracking: {:#?}, peer_id: {:?}", e, peer_id);
    }
}
