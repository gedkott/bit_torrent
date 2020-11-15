use std::collections::BTreeMap;
use std::fs::File;
use std::io::prelude::*;

mod bencode;
use bencode::*;

mod tracker;
use tracker::*;

#[derive(Debug)]
struct MetaInfoFile {
    info: Info,
    announce: String,
    announce_list: Option<Vec<Vec<String>>>,
    creation_date: Option<i32>,
    comment: Option<String>,
    created_by: Option<String>,
    encoding: Option<String>,
}

struct Info {
    piece_length: i32,
    pieces: Vec<u8>,
    private: Option<i32>,
}

impl std::fmt::Debug for Info {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let pieces =
            std::str::from_utf8(self.pieces.as_slice()).unwrap_or(&"BINARY_STRING_OF_BYTES");
        f.debug_struct("Info")
            .field("pieces_length", &self.piece_length)
            .field("private", &self.private)
            .field("pieces", &pieces)
            .finish()
    }
}

impl From<Bencodable> for MetaInfoFile {
    fn from(b: Bencodable) -> Self {
        let info = match &b {
            Bencodable::Dictionary(btm) => {
                let info_key = &BencodableByteString::from("info");
                match &btm[info_key] {
                    Bencodable::Dictionary(btm) => {
                        let piece_length_key = &BencodableByteString::from("piece length");
                        let pieces_key = &BencodableByteString::from("pieces");
                        let piece_length = match btm[piece_length_key] {
                            Bencodable::Integer(i) => i,
                            _ => panic!("did not find piece length"),
                        };

                        let pieces = match &btm[pieces_key] {
                            Bencodable::ByteString(bs) => bs,
                            _ => panic!("did not find pieces"),
                        };
                        Info {
                            piece_length,
                            pieces: pieces.0.to_owned(),
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
                    Bencodable::ByteString(bs) => std::str::from_utf8(&bs.0),
                    _ => panic!("did not find announce"),
                }
            }
            _ => panic!("did not find dictionary for Metainfo file structure"),
        };

        MetaInfoFile {
            info,
            announce: announce.unwrap().to_string(),
            announce_list: None,
            creation_date: None,
            comment: None,
            created_by: None,
            encoding: None,
        }
    }
}

const TORRENT_FILE: &'static str = "6201484321_f1a88ca2cb_b_archive.torrent";
const MY_TORRENT_COPY: &'static str = "myfile.torrent";

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
    // println!("{:?}", std::str::from_utf8(bytes_slice).unwrap());
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

    let meta_info = MetaInfoFile::from(bencodable.clone());

    println!("{:#?}", meta_info);

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

    println!("bencoded info {:#?}", bdecode(bencoded));

    let info_hash = {
        let mut hasher = sha1::Sha1::new();

        hasher.update(bencoded);

        let bytes = hasher.digest().bytes();
        println!("info hash bytes {:?}", bytes);

        let url_encoded =
            percent_encoding::percent_encode(&bytes, percent_encoding::NON_ALPHANUMERIC)
                .to_string();

        println!(
            "url encoded {} as bytes {:?}",
            url_encoded,
            url_encoded.as_bytes()
        );

        url_encoded
    };

    println!("{:?} {:?}", info_hash, peer_id);

    if let Some(e) = Tracker::new().track(
        &format!(
            "{}?info_hash={}&peer_id={}",
            &meta_info.announce, info_hash, peer_id
        ),
        TrackerRequestParameters {
            port: ClientPort(8999),
            uploaded: TotalBytes(0),
            downloaded: TotalBytes(0),
            left: TotalBytes(0),
            event: Event::Started,
        },
    )
    .and_then(|resp| {
        println!("Response {:#?}", resp);
        Ok(())
    })
    .err()
    {
        println!("Error from tracking: {:?}", e);
    }
}
