use crate::bencode::*;

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

pub struct Info<'a> {
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
