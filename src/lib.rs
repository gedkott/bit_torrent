use std::collections::BTreeMap;

#[derive(Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct BencodableByteString(Vec<u8>);

impl std::fmt::Debug for BencodableByteString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(std::str::from_utf8(self.0.as_slice()).unwrap_or(&"BYTES".to_string()))
    }
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub enum Bencodable {
    ByteString(BencodableByteString),
    Integer(i32),
    List(Vec<Bencodable>),
    Dictionary(BTreeMap<BencodableByteString, Bencodable>),
}

impl From<&str> for Bencodable {
    fn from(s: &str) -> Self {
        Bencodable::ByteString(BencodableByteString(s.as_bytes().to_vec()))
    }
}

impl From<&str> for BencodableByteString {
    fn from(s: &str) -> Self {
        BencodableByteString(s.as_bytes().to_vec())
    }
}

impl From<&[u8]> for Bencodable {
    fn from(b: &[u8]) -> Self {
        Bencodable::ByteString(BencodableByteString((b).to_vec()))
    }
}

#[derive(Debug)]
pub enum EncodeError {
    Generic,
    NotUTF8(std::str::Utf8Error),
}

pub fn bencode(b: &Bencodable) -> Result<Vec<u8>, EncodeError> {
    match b {
        Bencodable::ByteString(bs) => {
            let copy = bs.0.len().to_string();
            let mut buff = vec![copy.as_bytes()];
            buff.push(b":");
            buff.push(&bs.0);
            Ok(buff.into_iter().map(|x| x.to_owned()).flatten().collect())
        }
        Bencodable::Integer(int) => {
            let mut buff: Vec<Vec<u8>> = vec![b"i".to_vec()];
            let int = int.to_owned().to_string().as_bytes().to_owned();
            buff.push(int);
            buff.push(b"e".to_vec());
            Ok(buff.into_iter().flatten().collect())
        }
        Bencodable::List(lb) => {
            let mut bs = vec![];
            for b in lb {
                match bencode(b) {
                    Ok(bencodable) => {
                        bs.push(bencodable);
                    }
                    Err(e) => return Err(e),
                }
            }
            let bytes_of_bytes = bs.into_iter().flatten().collect::<Vec<u8>>();
            let mut buff = vec![b"l".to_vec()];
            buff.push(bytes_of_bytes);
            buff.push(b"e".to_vec());
            Ok(buff.into_iter().flatten().collect())
        }
        Bencodable::Dictionary(m) => {
            let mut bs = vec![];
            for (k, v) in m {
                match bencode(&Bencodable::ByteString(k.clone())) {
                    Ok(bencodable) => {
                        bs.push(bencodable);
                    }
                    Err(e) => return Err(e),
                }

                match bencode(v) {
                    Ok(bencodable) => {
                        bs.push(bencodable);
                    }
                    Err(e) => return Err(e),
                }
            }
            let bytes_of_bytes = bs.into_iter().flatten().collect::<Vec<u8>>();
            let mut buff = vec![b"d".to_vec()];
            buff.push(bytes_of_bytes);
            buff.push(b"e".to_vec());
            Ok(buff.into_iter().flatten().collect())
        }
    }
}

#[derive(Debug)]
pub struct ParseResult {
    pub index: usize,
    pub bencodable: Bencodable,
}

impl From<(usize, Bencodable)> for ParseResult {
    fn from(pr: (usize, Bencodable)) -> Self {
        ParseResult {
            index: pr.0,
            bencodable: pr.1,
        }
    }
}

#[derive(Debug)]
pub struct BencodeParseError;

fn parse_byte_string(
    index: usize,
    bencoded_value: &[u8],
) -> Result<ParseResult, BencodeParseError> {
    let mut i = index;
    let mut length_string = String::new();
    let mut next_char = bencoded_value[i];
    while next_char != b':' {
        i += 1;
        length_string.push(next_char as char);
        next_char = bencoded_value[i];
    }
    let length = length_string.parse::<usize>().unwrap();
    Ok(ParseResult::from((
        i + 1 + length, // +1 for the semicolon consumed
        Bencodable::from(&bencoded_value[i + 1..i + 1 + length]),
    )))
}

fn parse_integer(index: usize, bencoded_value: &[u8]) -> Result<ParseResult, BencodeParseError> {
    let mut i = index;
    let mut integer_string = String::new();
    let mut next_char = bencoded_value[i];
    while next_char != b'e' {
        i += 1;
        integer_string.push(next_char as char);
        next_char = bencoded_value[i];
    }
    let integer = integer_string.parse::<i32>().unwrap();
    // +1 for the last character consumed as partof parsing the bencodable ("e")
    Ok(ParseResult::from((i + 1, Bencodable::Integer(integer))))
}

fn parse_list(index: usize, bencoded_value: &[u8]) -> Result<ParseResult, BencodeParseError> {
    let mut i = index;
    let mut bencodables = vec![];
    let mut next_char = bencoded_value[i];
    while next_char != b'e' {
        let item = parse_bencoded_value(i, bencoded_value)?;
        bencodables.push(item.bencodable);
        i = item.index;
        next_char = bencoded_value[i];
    }
    // +1 for the last character consumed as partof parsing the bencodable ("e")
    let result = (i + 1, Bencodable::List(bencodables));
    Ok(ParseResult::from(result))
}

fn parse_dictionary(index: usize, bencoded_value: &[u8]) -> Result<ParseResult, BencodeParseError> {
    let mut i = index;
    let mut bencodables = BTreeMap::new();
    let mut next_char = bencoded_value[i];
    while next_char != b'e' {
        let byte_string_key =
            parse_bencoded_value(i, bencoded_value).and_then(|pr| match pr.bencodable {
                Bencodable::ByteString(bs) => Ok((pr.index, bs.0)),
                _ => Err(BencodeParseError),
            })?;
        let result = parse_bencoded_value(byte_string_key.0, bencoded_value)?;
        let key = BencodableByteString(byte_string_key.1);
        let value = result.bencodable;
        bencodables.insert(key, value);
        i = result.index;
        next_char = bencoded_value[i];
    }
    // +1 for the last character consumed as partof parsing the bencodable ("e")
    Ok(ParseResult::from((
        i + 1,
        Bencodable::Dictionary(bencodables),
    )))
}

fn parse_bencoded_value(
    index: usize,
    bencoded_value: &[u8],
) -> Result<ParseResult, BencodeParseError> {
    let i = index;
    let b = bencoded_value[i];
    if b.is_ascii_digit() {
        parse_byte_string(i, bencoded_value)
    } else if b == b'i' {
        parse_integer(i + 1, bencoded_value)
    } else if b == b'l' {
        parse_list(i + 1, bencoded_value)
    } else if b == b'd' {
        parse_dictionary(i + 1, bencoded_value)
    } else {
        Err(BencodeParseError)
    }
}

pub fn bdecode(bencoded_bytes: &[u8]) -> Result<Bencodable, BencodeParseError> {
    parse_bencoded_value(0, bencoded_bytes).map(|b| b.bencodable)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_encodes_simple_dictionaries() {
        let mut examples = BTreeMap::new();
        examples.insert(
            BencodableByteString::from("Gedalia"),
            Bencodable::from("Gedalia"),
        );
        examples.insert(BencodableByteString::from("a"), Bencodable::Integer(1));
        assert_eq!(
            bencode(&Bencodable::Dictionary(examples)).unwrap(),
            "d7:Gedalia7:Gedalia1:ai1ee".as_bytes()
        );
    }

    #[test]
    fn it_encodes_integers() {
        let result = bencode(&Bencodable::Integer(-311)).unwrap();
        let as_slice = result.as_slice();
        assert_eq!(as_slice, "i-311e".as_bytes());
    }

    #[test]
    fn it_encodes_byte_strings() {
        assert_eq!(
            bencode(&Bencodable::from("Gedalia")).unwrap(),
            "7:Gedalia".as_bytes()
        );
    }

    #[test]
    fn it_encodes_long_byte_strings() {
        assert_eq!(
            bencode(&Bencodable::from("GedaliaGedalia")).unwrap(),
            "14:GedaliaGedalia".as_bytes()
        );
    }

    #[test]
    fn it_encodes_lists() {
        assert_eq!(
            "l4:spam4:eggsi-341ee",
            std::str::from_utf8(
                &bencode(&Bencodable::List(vec!(
                    Bencodable::from("spam"),
                    Bencodable::from("eggs"),
                    Bencodable::Integer(-341)
                )))
                .unwrap()
            )
            .unwrap()
        );
    }

    #[test]
    fn it_encodes_more_complex_dictionaries() {
        let mut examples = BTreeMap::new();
        examples.insert(
            BencodableByteString::from("publisher"),
            Bencodable::from("bob"),
        );
        examples.insert(
            BencodableByteString::from("publisher-webpage"),
            Bencodable::from("www.example.com"),
        );
        examples.insert(
            BencodableByteString::from("publisher.location"),
            Bencodable::from("home"),
        );
        assert_eq!(
            "d9:publisher3:bob17:publisher-webpage15:www.example.com18:publisher.location4:homee",
            std::str::from_utf8(&bencode(&Bencodable::Dictionary(examples)).unwrap()).unwrap()
        );
    }

    #[test]
    fn it_decodes_byte_strings() {
        assert_eq!(bdecode(b"4:spam").unwrap(), Bencodable::from("spam"));
    }

    #[test]
    fn it_decodes_long_byte_strings() {
        assert_eq!(
            bdecode(b"14:GedaliaGedalia").unwrap(),
            Bencodable::from("GedaliaGedalia")
        );
    }

    #[test]
    fn it_decodes_lists() {
        assert_eq!(bdecode(b"i-3e").unwrap(), Bencodable::Integer(-3));
    }

    #[test]
    fn it_decodes_integers() {
        assert_eq!(bdecode(b"i-341e").unwrap(), Bencodable::Integer(-341));
    }

    #[test]
    fn it_decodes_small_integers() {
        assert_eq!(bdecode(b"i3e").unwrap(), Bencodable::Integer(3));
    }

    #[test]
    fn it_decodes_heterogenous_lists() {
        assert_eq!(
            bdecode(b"l4:spam4:eggsi-341ee").unwrap(),
            Bencodable::List(vec!(
                Bencodable::from("spam"),
                Bencodable::from("eggs"),
                Bencodable::Integer(-341)
            ))
        );
    }

    #[test]
    fn it_decodes_dictionaries() {
        let mut examples = BTreeMap::new();
        examples.insert(
            BencodableByteString::from("Gedalia"),
            Bencodable::from("Gedalia"),
        );
        examples.insert(BencodableByteString::from("a"), Bencodable::Integer(1));
        assert_eq!(
            bdecode(b"d7:Gedalia7:Gedalia1:ai1ee").unwrap(),
            Bencodable::Dictionary(examples)
        );
    }

    #[test]
    fn it_decodes_dictionaries_with_embedded_list_values() {
        let mut examples = BTreeMap::new();
        examples.insert(
            BencodableByteString::from("spam"),
            Bencodable::List(vec![Bencodable::from("a"), Bencodable::from("b")]),
        );
        assert_eq!(
            bdecode(b"d4:spaml1:a1:bee").unwrap(),
            Bencodable::Dictionary(examples)
        );
    }

    #[test]
    fn it_decodes_complex_dictionaries() {
        let mut examples = BTreeMap::new();
        examples.insert(
            BencodableByteString::from("publisher"),
            Bencodable::from("bob"),
        );
        examples.insert(
            BencodableByteString::from("publisher-webpage"),
            Bencodable::from("www.example.com"),
        );
        examples.insert(
            BencodableByteString::from("publisher.location"),
            Bencodable::from("home"),
        );
        assert_eq!(
            bdecode(b"d9:publisher3:bob17:publisher-webpage15:www.example.com18:publisher.location4:homee").unwrap(),
            Bencodable::Dictionary(examples)
        );
    }

    #[test]
    fn it_decodes_empty_dictionaries() {
        assert_eq!(
            bdecode(b"de").unwrap(),
            Bencodable::Dictionary(BTreeMap::new())
        );
    }

    #[test]
    fn it_decodes_empty_lists() {
        assert_eq!(bdecode(b"le").unwrap(), Bencodable::List(Vec::new()));
    }

    #[test]
    fn it_decodes_lists_inside_lists_inside_maps() {
        let mut examples = BTreeMap::new();
        examples.insert(
            BencodableByteString::from("announce"),
            Bencodable::from("udp://tracker.leechers-paradise.org:6969"),
        );
        examples.insert(
            BencodableByteString::from("announce-list"),
            Bencodable::List(vec![
                Bencodable::List(vec![Bencodable::from(
                    "udp://tracker.leechers-paradise.org:6969",
                )]),
                Bencodable::List(vec![Bencodable::from("udp://tracker.coppersurfer.tk:6969")]),
            ]),
        );

        let example_string = vec![
            "d",                                           //dict-start
            "8:announce",                                  //key announce
            "40:udp://tracker.leechers-paradise.org:6969", //value bytestring URL
            "13:announce-list",                            // key announce-list
            "l",                                           // list-start
            "l",                                           // list-start
            "40:udp://tracker.leechers-paradise.org:6969", //value bytestring URL
            "e",                                           // list-end
            "l",                                           //list-start
            "34:udp://tracker.coppersurfer.tk:6969",       // value bytestring URL
            "e",                                           // list-end
            "e",                                           // list-end
            "e",                                           // dict-end
        ]
        .join("");

        let t = bdecode(example_string.as_bytes());
        assert_eq!(t.unwrap(), Bencodable::Dictionary(examples));
    }
}
