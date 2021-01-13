use crate::bencode::*;
use sha1::Sha1;

#[derive(Debug)]
pub struct MetaInfoFile {
    info: Info,
    pub announce: String,
    announce_list: Option<Vec<Vec<String>>>,
    creation_date: Option<i32>,
    comment: Option<String>,
    created_by: Option<String>,
    encoding: Option<String>,
}

impl MetaInfoFile {
    pub fn pieces(&self) -> &[String] {
        &self.info.pieces
    }

    pub fn piece_length(&self) -> i32 {
        self.info.piece_length
    }

    pub fn file_length(&self) -> i32 {
        match &self.info.files {
            Files::File(f) => f.length,
        }
    }
}

#[derive(Debug)]
struct File {
    length: i32,
    path: String,
}

#[derive(Debug)]
enum Files {
    File(File),
}

#[derive(Debug)]
pub struct Info {
    piece_length: i32,
    pieces: Vec<String>,
    private: Option<i32>,
    name: String,
    files: Files,
}

impl<'a> From<&'a Bencodable> for MetaInfoFile {
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
                            name: name.to_string(),
                            files: Files::File(File {
                                length: *length,
                                path: name.to_string(),
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
            announce: announce.unwrap().to_string(),
            announce_list: None,
            creation_date: None,
            comment: None,
            created_by: None,
            encoding: None,
        }
    }
}
