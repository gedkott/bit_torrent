use crate::bencode::*;
use sha1::Sha1;

#[derive(Debug)]
pub struct MetaInfoFile<'a> {
    info: Info<'a>,
    pub announce: &'a str,
    announce_list: Option<Vec<Vec<String>>>,
    creation_date: Option<i32>,
    comment: Option<String>,
    created_by: Option<String>,
    encoding: Option<String>,
}

#[derive(Debug)]
struct File<'a> {
    length: i32,
    path: &'a str,
}

#[derive(Debug)]
enum Files<'a> {
    File(File<'a>),
}

#[derive(Debug)]
pub struct Info<'a> {
    piece_length: i32,
    pieces: Vec<String>,
    private: Option<i32>,
    name: &'a str,
    files: Files<'a>,
}

impl<'a> From<&'a Bencodable> for MetaInfoFile<'a> {
    fn from(b: &'a Bencodable) -> Self {
        let info = match &b {
            Bencodable::Dictionary(btm) => {
                let info_key = &BencodableByteString::from("info");
                match &btm[info_key] {
                    Bencodable::Dictionary(btm) => {
                        // in current example, we see 131072 => log base 2 of 131072 = 17
                        // (since spec says the piece length is almost always a power of 2)
                        let piece_length_key = &BencodableByteString::from("piece length");
                        let piece_length = match btm[piece_length_key] {
                            Bencodable::Integer(i) => i,
                            _ => panic!("did not find `piece length`"),
                        };

                        let pieces_key = &BencodableByteString::from("pieces");
                        let pieces: Vec<String> = match &btm[pieces_key] {
                            Bencodable::ByteString(bs) => bs
                                .as_bytes()
                                .chunks(20)
                                .map(|c| Sha1::from(c).hexdigest())
                                .collect(),
                            _ => panic!("did not find `pieces`"),
                        };

                        let name_key = &BencodableByteString::from("name");
                        let name = match &btm[name_key] {
                            Bencodable::ByteString(bs) => bs.as_string().unwrap(),
                            _ => panic!("did not find `name`"),
                        };

                        let length_key = &BencodableByteString::from("length");
                        let length = match &btm[length_key] {
                            Bencodable::Integer(i) => i,
                            _ => panic!("did not find `length` (expected to find `files` instead)"),
                        };

                        Info {
                            piece_length,
                            pieces,
                            private: None,
                            name,
                            files: Files::File(File {
                                length: *length,
                                path: name,
                            }),
                        }
                    }
                    _ => panic!("did not find `info`"),
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
