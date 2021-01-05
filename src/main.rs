use rand::Rng;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::prelude::*;
use std::sync::{Arc, Mutex};
use std::{thread, time};

use sha1::Sha1;

mod bencode;
use bencode::*;

mod bit_torrent_data;
use bit_torrent_data::*;

mod tracker;
use tracker::*;

mod messages;
use messages::*;

mod peer_tcp_client;
use peer_tcp_client::*;

const TORRENT_FILE: &str = "Charlie_Chaplin_Mabels_Strange_Predicament.avi.torrent";
const MY_TORRENT_COPY: &str = "myfile.torrent";

fn main() {
    let mut examples = BTreeMap::new();
    examples.insert(
        BencodableByteString::from("Gedalia"),
        Bencodable::from("Gedalia"),
    );
    examples.insert(BencodableByteString::from("a"), Bencodable::Integer(1));
    assert_eq!(
        bencode(&Bencodable::Dictionary(examples)).unwrap(),
        b"d7:Gedalia7:Gedalia1:ai1ee".to_vec()
    );

    assert_eq!(bdecode(b"4:spam").unwrap(), Bencodable::from("spam"));

    let mut f = File::open(TORRENT_FILE).unwrap();
    let mut bytes = Vec::new();
    f.read_to_end(&mut bytes).unwrap();
    let bytes_slice = bytes.as_slice();
    let decoded_original = bdecode(bytes_slice).unwrap();

    File::create(MY_TORRENT_COPY)
        .and_then(|mut f| f.write_all(bencode(&decoded_original).unwrap().as_slice()))
        .ok();

    let mut f = File::open(MY_TORRENT_COPY).unwrap();
    let mut bytes = Vec::new();
    f.read_to_end(&mut bytes).unwrap();

    let decoded_from_new_file_written_with_encoded_original = bdecode(bytes.as_slice()).unwrap();

    assert_eq!(
        decoded_original,
        decoded_from_new_file_written_with_encoded_original
    );

    let bencodable = decoded_from_new_file_written_with_encoded_original;

    let meta_info = MetaInfoFile::from(&bencodable);

    println!("{:?}", meta_info);

    let peer_id: String = rand::thread_rng().gen_ascii_chars().take(20).collect();

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

    let bencoded = &info.unwrap();

    let info_hash = {
        let mut hasher = Sha1::new();

        hasher.update(bencoded);

        hasher.digest().bytes()
    };

    let info_encoded = {
        percent_encoding::percent_encode(&info_hash, percent_encoding::NON_ALPHANUMERIC).to_string()
    };

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
                .take(6)
                .map(|tp| match tp {
                    TrackerPeer::Peer(p) => p,
                    TrackerPeer::SocketAddr(sa) => {
                        println!(
                            "weird peer from tracker with only socket addr, no ID: {:?}",
                            sa
                        );
                        let id: String = rand::thread_rng().gen_ascii_chars().take(20).collect();
                        tracker::Peer {
                            id: id.as_bytes().to_vec(),
                            socket_addr: sa,
                        }
                    }
                })
                .collect::<Vec<tracker::Peer>>();
            PeerTcpClient::connect(&tcp_peers_w_peer_id, &info_hash)
        })
        .map(|ptc| {
            let r = ptc.listen();
            let mut streams: Vec<Arc<Mutex<Stream>>> =
                r.threads.into_iter().map(|(_, s)| s).collect();
            for stream in &mut streams {
                println!("handshaking with {:?}", stream);
                stream.lock().unwrap().handshake(&info_hash);
            }
            let message_receiver = r.receiver;

            loop {
                println!("waiting for message...");
                let (stream, message) = message_receiver.recv().unwrap();
                let message = message.unwrap();
                println!("message: {:?}", message);
                match message {
                    Message::BitField(_) => {}
                    Message::Choke => {
                        stream.lock().unwrap().choke_self();
                        stream.lock().unwrap().interested();
                    }
                    Message::UnChoke => stream.lock().unwrap().unchoke_self(),
                    _ => (),
                }
                let ten_millis = time::Duration::from_millis(1000);
                thread::sleep(ten_millis);
            }
        })
        .err()
    {
        println!("Error from tracking: {:#?}, peer_id: {:?}", e, peer_id);
    }
}
