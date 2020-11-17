use std::collections::BTreeMap;

#[derive(Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct BencodableByteString(Vec<u8>);

impl std::fmt::Debug for BencodableByteString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            std::str::from_utf8(self.0.as_slice()).unwrap_or(&format!("{:02X?}", self.as_bytes())),
        )
    }
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub enum Bencodable {
    ByteString(BencodableByteString),
    Integer(i32),
    List(Vec<Bencodable>),
    Dictionary(BTreeMap<BencodableByteString, Bencodable>),
}

impl BencodableByteString {
    pub fn as_string(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(&self.0)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl From<&str> for Bencodable {
    fn from(s: &str) -> Self {
        Bencodable::ByteString(BencodableByteString::from(s))
    }
}

impl From<&str> for BencodableByteString {
    fn from(s: &str) -> Self {
        BencodableByteString(s.as_bytes().to_vec())
    }
}

impl From<&[u8]> for BencodableByteString {
    fn from(s: &[u8]) -> Self {
        BencodableByteString(s.to_vec())
    }
}

impl From<&[u8]> for Bencodable {
    fn from(b: &[u8]) -> Self {
        Bencodable::ByteString(BencodableByteString((b).to_vec()))
    }
}

#[derive(Debug)]
pub enum EncodeError {
    ListEncodeFailure,
    DictKeyEncodeFailure,
    DictValueEncodeFailure,
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
                    Err(_) => return Err(EncodeError::ListEncodeFailure),
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
                    Err(_) => return Err(EncodeError::DictKeyEncodeFailure),
                }

                match bencode(v) {
                    Ok(bencodable) => {
                        bs.push(bencodable);
                    }
                    Err(_) => return Err(EncodeError::DictValueEncodeFailure),
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

#[derive(Debug, PartialEq, Eq)]
pub struct BencodeParseError {
    index: usize,
    original: String,
    error_type: BencodeParseErrorType,
}

#[derive(Debug, PartialEq, Eq)]
pub enum BencodeParseErrorType {
    ParseInteger,
    ParseList,
    ParseDictionary,
    ParseByteString,
    ParseByteStringLength,
    ParseInitiate,
    ParseEnd,
    ParseValue,
}

impl From<(BencodeParseErrorType, usize, &[u8])> for BencodeParseError {
    fn from(t: (BencodeParseErrorType, usize, &[u8])) -> Self {
        BencodeParseError {
            error_type: t.0,
            index: t.1,
            original: std::str::from_utf8(t.2).unwrap_or("BYTES").to_string(),
        }
    }
}

fn parse_byte_string(
    index: usize,
    bencoded_value: &[u8],
) -> Result<ParseResult, BencodeParseError> {
    let mut i = index;
    let mut length_string = String::new();
    let mut next_char = *bencoded_value.get(i).ok_or_else(|| {
        BencodeParseError::from((BencodeParseErrorType::ParseByteString, i, bencoded_value))
    })?;
    while next_char != b':' {
        i += 1;
        length_string.push(next_char as char);
        next_char = *bencoded_value.get(i).ok_or_else(|| {
            BencodeParseError::from((
                BencodeParseErrorType::ParseByteStringLength,
                i,
                bencoded_value,
            ))
        })?;
    }
    let length = length_string.parse::<usize>().map_err(|_| {
        BencodeParseError::from((
            BencodeParseErrorType::ParseByteStringLength,
            i,
            bencoded_value,
        ))
    })?;
    let relevant_slice = bencoded_value.get(i + 1..i + 1 + length).ok_or_else(|| {
        BencodeParseError::from((BencodeParseErrorType::ParseByteString, i, bencoded_value))
    })?;
    let bencodable = Bencodable::from(relevant_slice);
    Ok(ParseResult::from((
        i + 1 + length, // +1 for the semicolon consumed
        bencodable,
    )))
}

fn parse_integer(index: usize, bencoded_value: &[u8]) -> Result<ParseResult, BencodeParseError> {
    let mut i = index;
    let mut integer_string = String::new();
    let mut next_char = *bencoded_value.get(i).ok_or_else(|| {
        BencodeParseError::from((BencodeParseErrorType::ParseInteger, i, bencoded_value))
    })?;
    while next_char != b'e' {
        i += 1;
        integer_string.push(next_char as char);
        next_char = *bencoded_value.get(i).ok_or_else(|| {
            BencodeParseError::from((BencodeParseErrorType::ParseInteger, i, bencoded_value))
        })?;
    }
    let integer = integer_string.parse::<i32>().map_err(|_| {
        BencodeParseError::from((BencodeParseErrorType::ParseInteger, i, bencoded_value))
    })?;
    // +1 for the last character consumed as part of parsing the bencodable ("e")
    Ok(ParseResult::from((i + 1, Bencodable::Integer(integer))))
}

fn parse_list(index: usize, bencoded_value: &[u8]) -> Result<ParseResult, BencodeParseError> {
    let mut i = index;
    let mut bencodables = vec![];
    let mut next_char = *bencoded_value.get(i).ok_or_else(|| {
        BencodeParseError::from((BencodeParseErrorType::ParseList, i, bencoded_value))
    })?;
    while next_char != b'e' {
        let item = parse_bencoded_value(i, bencoded_value)?;
        bencodables.push(item.bencodable);
        i = item.index;
        next_char = *bencoded_value.get(i).ok_or_else(|| {
            BencodeParseError::from((BencodeParseErrorType::ParseList, i, bencoded_value))
        })?;
    }
    // +1 for the last character consumed as part of parsing the bencodable ("e")
    let result = (i + 1, Bencodable::List(bencodables));
    Ok(ParseResult::from(result))
}

fn parse_dictionary(index: usize, bencoded_value: &[u8]) -> Result<ParseResult, BencodeParseError> {
    let mut i = index;
    let mut bencodables = BTreeMap::new();
    let mut next_char = *bencoded_value.get(i).ok_or_else(|| {
        BencodeParseError::from((BencodeParseErrorType::ParseDictionary, i, bencoded_value))
    })?;
    while next_char != b'e' {
        let byte_string_key =
            parse_bencoded_value(i, bencoded_value).and_then(|pr| match pr.bencodable {
                Bencodable::ByteString(bs) => Ok((pr.index, bs.0)),
                _ => Err(BencodeParseError::from((
                    BencodeParseErrorType::ParseDictionary,
                    i,
                    bencoded_value,
                ))),
            })?;
        let result = parse_bencoded_value(byte_string_key.0, bencoded_value)?;
        let key = BencodableByteString(byte_string_key.1);
        let value = result.bencodable;
        bencodables.insert(key, value);
        i = result.index;
        next_char = *bencoded_value.get(i).ok_or_else(|| {
            BencodeParseError::from((
                BencodeParseErrorType::ParseByteStringLength,
                i,
                bencoded_value,
            ))
        })?;
    }
    // +1 for the last character consumed as part of parsing the bencodable ("e")
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
    let b = *bencoded_value.get(i).ok_or_else(|| {
        BencodeParseError::from((BencodeParseErrorType::ParseValue, i, bencoded_value))
    })?;
    if b.is_ascii_digit() {
        parse_byte_string(i, bencoded_value)
    } else if b == b'i' {
        parse_integer(i + 1, bencoded_value)
    } else if b == b'l' {
        parse_list(i + 1, bencoded_value)
    } else if b == b'd' {
        parse_dictionary(i + 1, bencoded_value)
    } else {
        Err(BencodeParseError::from((
            BencodeParseErrorType::ParseInitiate,
            i,
            bencoded_value,
        )))
    }
}

pub fn bdecode(bencoded_bytes: &[u8]) -> Result<Bencodable, BencodeParseError> {
    parse_bencoded_value(0, bencoded_bytes)
        .and_then(|pr: ParseResult| {
            let next_index = pr.index;
            if bencoded_bytes.get(next_index).is_some() {
                Err(BencodeParseError::from((
                    BencodeParseErrorType::ParseEnd,
                    next_index,
                    bencoded_bytes,
                )))
            } else {
                Ok(pr)
            }
        })
        .map(|b| b.bencodable)
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
    fn it_decodes_empty_bytes() {
        assert_eq!(
            bdecode(b""),
            Err(BencodeParseError::from((
                BencodeParseErrorType::ParseValue,
                0 as usize,
                "".as_bytes()
            )))
        );
    }

    #[test]
    fn it_decodes_incomplete_bencode() {
        assert_eq!(
            bdecode(b"d9:publisher3:bob17:publisher-webpage1:www.example.com18:publisher.location4:homee"),
            Err(BencodeParseError::from((BencodeParseErrorType::ParseInitiate, 40 as usize, "d9:publisher3:bob17:publisher-webpage1:www.example.com18:publisher.location4:homee".as_bytes())))
        );
    }

    #[test]
    fn it_decodes_incomplete_dictionaries() {
        assert_eq!(
            bdecode(b"d"),
            Err(BencodeParseError::from((
                BencodeParseErrorType::ParseDictionary,
                1 as usize,
                "d".as_bytes()
            )))
        );
    }

    #[test]
    fn it_decodes_incomplete_lists() {
        assert_eq!(
            bdecode(b"li3e"),
            Err(BencodeParseError::from((
                BencodeParseErrorType::ParseList,
                4 as usize,
                "li3e".as_bytes()
            )))
        );
    }

    #[test]
    fn it_decodes_incomplete_integers() {
        assert_eq!(
            bdecode(b"i311111111111d"),
            Err(BencodeParseError::from((
                BencodeParseErrorType::ParseInteger,
                14 as usize,
                "i311111111111d".as_bytes()
            )))
        );
    }

    #[test]
    fn it_decodes_incomplete_byte_strings() {
        assert_eq!(
            bdecode(b"2:a"),
            Err(BencodeParseError::from((
                BencodeParseErrorType::ParseByteString,
                1 as usize,
                "2:a".as_bytes()
            )))
        );
    }

    #[test]
    fn it_decodes_too_long_length_byte_strings() {
        assert_eq!(
            bdecode(b"2:abc"),
            Err(BencodeParseError::from((
                BencodeParseErrorType::ParseEnd,
                4 as usize,
                "2:abc".as_bytes()
            )))
        );
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
            "d",                                           // dict-start
            "8:announce",                                  // key announce
            "40:udp://tracker.leechers-paradise.org:6969", // value bytestring URL
            "13:announce-list",                            // key announce-list
            "l",                                           // list-start
            "l",                                           // list-start
            "40:udp://tracker.leechers-paradise.org:6969", // value bytestring URL
            "e",                                           // list-end
            "l",                                           // list-start
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
