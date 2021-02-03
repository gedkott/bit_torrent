use crate::bencode::*;
use sha1::Sha1;
use std::fs::File as FsFile;
use std::io::prelude::*;

#[derive(Debug)]
pub struct MetaInfoFile {
    info: Info,
    pub announce: String,
    announce_list: Option<Vec<Vec<String>>>,
    creation_date: Option<u32>,
    comment: Option<String>,
    created_by: Option<String>,
    encoding: Option<String>,
    pub info_hash: [u8; 20],
}

impl MetaInfoFile {
    pub fn file_name(&self) -> String {
        self.info.name.clone()
    }
}

impl crate::PiecedContent for MetaInfoFile {
    fn number_of_pieces(&self) -> u32 {
        self.info.pieces.len() as u32
    }

    fn piece_length(&self) -> u32 {
        self.info.piece_length
    }

    fn name(&self) -> String {
        self.file_name()
    }

    fn total_length(&self) -> u32 {
        match &self.info.files {
            Files::File(f) => f.length,
        }
    }
}

#[derive(Debug)]
struct File {
    length: u32,
    path: String,
}

#[derive(Debug)]
enum Files {
    File(File),
}

#[derive(Debug)]
pub struct Info {
    piece_length: u32,
    pieces: Vec<String>,
    private: Option<u32>,
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
                        let length = match &btm.get(length_key) {
                            Some(Bencodable::Integer(i)) => i,
                            _ => panic!(
                                "did not find `length` (expected to find `files` instead): {:?}",
                                b
                            ),
                        };

                        Info {
                            piece_length: piece_length as u32,
                            pieces,
                            private: None,
                            name: name.to_string(),
                            files: Files::File(File {
                                length: *length as u32,
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

        let info_hash = {
            let info = match &b {
                Bencodable::Dictionary(btm) => {
                    let info_key = &BencodableByteString::from("info");
                    match &btm[info_key] {
                        Bencodable::Dictionary(btm) => {
                            bencode(&Bencodable::Dictionary(btm.clone()))
                        }
                        _ => panic!("did not find info for info hash"),
                    }
                }
                _ => panic!("did not find dictionary for Metainfo file structure for info hash"),
            };
            let mut hasher = Sha1::new();
            hasher.update(&info.unwrap());
            hasher.digest().bytes()
        };

        MetaInfoFile {
            info,
            announce: announce.unwrap().to_string(),
            announce_list: None,
            creation_date: None,
            comment: None,
            created_by: None,
            encoding: None,
            info_hash,
        }
    }
}

impl From<FsFile> for MetaInfoFile {
    fn from(mut f: FsFile) -> Self {
        let mut bytes = Vec::new();
        f.read_to_end(&mut bytes).unwrap();
        let bytes_slice = bytes.as_slice();
        let bencodable = bdecode(bytes_slice).unwrap();
        MetaInfoFile::from(&bencodable)
    }
}
