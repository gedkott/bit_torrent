use std::collections::BTreeMap;
use std::fs::File;
use std::io::prelude::*;

mod bencode;
use bencode::*;

mod tracker;
use tracker::*;

#[derive(Debug)]
struct MetaInfoFile<'a> {
    info: Info<'a>,
    announce: &'a str,
    announce_list: Option<Vec<Vec<String>>>,
    creation_date: Option<i32>,
    comment: Option<String>,
    created_by: Option<String>,
    encoding: Option<String>,
}

struct Info<'a> {
    piece_length: i32,
    pieces: &'a [u8],
    private: Option<i32>,
}

impl std::fmt::Debug for Info<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let pieces = std::str::from_utf8(&self.pieces).unwrap_or("BYTES");
        f.debug_struct("Info")
            .field("pieces_length", &self.piece_length)
            .field("private", &self.private)
            .field("pieces", &pieces)
            .finish()
    }
}

impl<'a> From<&'a Bencodable> for MetaInfoFile<'a> {
    fn from(b: &'a Bencodable) -> Self {
        let info = match &b {
            Bencodable::Dictionary(btm) => {
                let info_key = &BencodableByteString::from("info");
                match &btm[info_key] {
                    Bencodable::Dictionary(btm) => {
                        let piece_length_key = &BencodableByteString::from("piece length");
                        let piece_length = match btm[piece_length_key] {
                            Bencodable::Integer(i) => i,
                            _ => panic!("did not find piece length"),
                        };

                        let pieces_key = &BencodableByteString::from("pieces");
                        let pieces = match &btm[pieces_key] {
                            Bencodable::ByteString(bs) => bs.as_bytes(),
                            _ => panic!("did not find pieces"),
                        };

                        Info {
                            piece_length,
                            pieces,
                            private: None,
                        }
                    }
                    _ => panic!("did not find info"),
                }
            }
            _ => panic!("did not find dictionary for Metainfo file structure"),
        };

        let announce = match &b {
            Bencodable::Dictionary(btm) => {
                let info_key = &BencodableByteString::from("announce");
                match &btm[info_key] {
                    Bencodable::ByteString(bs) => bs.as_string(),
                    _ => panic!("did not find announce"),
                }
            }
            _ => panic!("did not find dictionary for Metainfo file structure"),
        };

        MetaInfoFile {
            info,
            announce: announce.unwrap(),
            announce_list: None,
            creation_date: None,
            comment: None,
            created_by: None,
            encoding: None,
        }
    }
}

const TORRENT_FILE: &str = "6201484321_f1a88ca2cb_b_archive.torrent";
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

    let peer_id = { "-qB4030-i.52DyS4ir)l" };

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
        let mut hasher = sha1::Sha1::new();

        hasher.update(bencoded);

        let bytes = hasher.digest().bytes();

        let url_encoded =
            percent_encoding::percent_encode(&bytes, percent_encoding::NON_ALPHANUMERIC)
                .to_string();

        url_encoded
    };

    if let Some(e) = Tracker::new()
        .track(
            &format!(
                "{}?info_hash={}&peer_id={}",
                &meta_info.announce, info_hash, peer_id
            ),
            TrackerRequestParameters {
                port: 8999,
                uploaded: 0,
                downloaded: 0,
                left: 0,
                event: Event::Started,
            },
        )
        .map(|resp| println!("Response {:#?}", resp))
        .err()
    {
        println!("Error from tracking: {:?}", e);
    }
}
