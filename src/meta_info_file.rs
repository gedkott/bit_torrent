use crate::bencode::*;
use crate::PiecedContent;
use sha1::{Digest, Sha1};
use std::collections::BTreeMap;
use std::fs::File as FsFile;
use std::io::prelude::*;

#[derive(Debug)]
pub struct File {
    pub length: u32,
    pub path: String,
}

pub struct Pieces(Vec<String>);

impl std::fmt::Debug for Pieces {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Pieces: {}", self.0.len())
    }
}

#[derive(Debug)]
pub enum Info {
    SingleFile {
        piece_length: u32,
        pieces: Pieces,
        name: String,
        file: File,
    },
    MultiFile {
        piece_length: u32,
        pieces: Pieces,
        directory_name: String,
        files: Vec<File>,
    },
}

#[derive(Debug)]
pub struct MetaInfoFile {
    pub info: Info,
    pub announce: String,
    pub info_hash: [u8; 20],
}

impl PiecedContent for MetaInfoFile {
    fn number_of_pieces(&self) -> u32 {
        match &self.info {
            Info::SingleFile {
                piece_length: _,
                pieces,
                name: _,
                file: _,
            } => pieces.0.len() as u32,
            Info::MultiFile {
                piece_length: _,
                pieces,
                directory_name: _,
                files: _,
            } => pieces.0.len() as u32,
        }
    }

    fn piece_length(&self) -> u32 {
        match &self.info {
            Info::SingleFile {
                piece_length,
                pieces: _,
                name: _,
                file: _,
            } => *piece_length,
            Info::MultiFile {
                piece_length,
                pieces: _,
                directory_name: _,
                files: _,
            } => *piece_length,
        }
    }

    fn total_length(&self) -> u32 {
        match &self.info {
            Info::SingleFile {
                piece_length: _,
                pieces: _,
                name: _,
                file,
            } => file.length,
            Info::MultiFile {
                piece_length: _,
                pieces: _,
                directory_name: _,
                files,
            } => files.iter().map(|f| f.length).sum(),
        }
    }
}

#[derive(Debug)]
enum MetaInfoFileParseError<'a> {
    GenericError(&'a str),
}

fn get_info_from_btm(
    btm: &BTreeMap<BencodableByteString, Bencodable>,
) -> Result<Info, MetaInfoFileParseError> {
    let piece_length_key = &BencodableByteString::from("piece length");
    let piece_length = match btm[piece_length_key] {
        Bencodable::Integer(i) => i,
        _ => {
            return Err(MetaInfoFileParseError::GenericError(
                "did not find `piece length`",
            ))
        }
    };

    let pieces_key = &BencodableByteString::from("pieces");
    let pieces: Vec<String> = match &btm[pieces_key] {
        Bencodable::ByteString(bs) => bs
            .as_bytes()
            .chunks(20)
            .map(|c| {
                let chars = <[u8; 20]>::from(Sha1::digest(c));
                hex::encode(chars)
            })
            .collect(),
        _ => {
            return Err(MetaInfoFileParseError::GenericError(
                "did not find `pieces`",
            ))
        }
    };

    let name_key = &BencodableByteString::from("name");
    let name = match &btm[name_key] {
        Bencodable::ByteString(bs) => bs.as_string().unwrap(),
        _ => return Err(MetaInfoFileParseError::GenericError("did not find `name`")),
    };

    let length_key = &BencodableByteString::from("length");
    // TODO(): Need to implement multiple files to download larger charlie chaplin torrent as a test...
    let length = match &btm.get(length_key) {
        Some(Bencodable::Integer(i)) => Some(i),
        _ => None,
    };

    if let Some(l) = length {
        Ok(Info::SingleFile {
            piece_length,
            pieces: Pieces(pieces),
            name: name.to_string(),
            file: File {
                length: *l,
                path: name.to_string(),
            },
        })
    } else {
        let files_key = &BencodableByteString::from("files");
        let files: Vec<File> = match &btm[files_key] {
            Bencodable::List(bs) => bs,
            _ => {
                panic!("did not find `files` when `length` was unavailable")
            }
        }
        .iter()
        .map(|b| -> Result<File, MetaInfoFileParseError> {
            println!("processing file bencodable {:?}\n", b);
            // crc32: ByteString(3481f090)
            // length: Integer(57772860)
            // md5: ByteString(bd8a51ac77e546826af44ff8396a69aa)
            // mtime: ByteString(1627109655)
            // path: List([ByteString(Charlie Chaplin . Mabel's Strange Predicament (1914 Restored Short Silent Film Noir Comedy).mp4)])
            // sha1: ByteString(720b65c5f3910b8d48b15a08b55417cb4f2ebf4a)
            match &b {
                Bencodable::Dictionary(btm) => {
                    let length_key = &BencodableByteString::from("length");
                    let length = match btm[length_key] {
                        Bencodable::Integer(i) => i,
                        _ => {
                            return Err(MetaInfoFileParseError::GenericError(
                                "did not find `length` for file in multifile torrent",
                            ))
                        }
                    };

                    let path_key = &BencodableByteString::from("path");
                    let path = match &btm[path_key] {
                        Bencodable::List(bs) => bs
                            .iter()
                            .map(|b| match &b {
                                Bencodable::ByteString(s) => s.as_string().unwrap().to_string(),
                                _ => {
                                    panic!("could not construct path for file in multifile torrent")
                                }
                            })
                            .collect::<Vec<String>>()
                            .join("\\"),
                        _ => {
                            return Err(MetaInfoFileParseError::GenericError(
                                "did not find `path` for file in multifile torrent",
                            ))
                        }
                    };

                    Ok(File { path, length })
                }
                _ => panic!("did not find `info`"),
            }
        })
        .map(|rf| rf.unwrap())
        .collect();
        Ok(Info::MultiFile {
            piece_length,
            pieces: Pieces(pieces),
            directory_name: name.to_string(),
            files,
        })
    }
}

fn get_info(b: &Bencodable) -> Result<Info, MetaInfoFileParseError> {
    match &b {
        Bencodable::Dictionary(btm) => {
            let info_key = &BencodableByteString::from("info");
            match &btm[info_key] {
                Bencodable::Dictionary(btm) => {
                    // in current example, we see 131072 => log base 2 of 131072 = 17
                    // (since spec says the piece length is almost always a power of 2)
                    get_info_from_btm(btm)
                }
                _ => panic!("did not find `info`"),
            }
        }
        _ => panic!("did not find dictionary for Metainfo file structure"),
    }
}

impl<'a> From<&'a Bencodable> for MetaInfoFile {
    fn from(b: &'a Bencodable) -> Self {
        let info = get_info(b).unwrap();

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
            <[u8; 20]>::from(hasher.finalize())
        };

        MetaInfoFile {
            info,
            announce: announce.unwrap().to_string(),
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
