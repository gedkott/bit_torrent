use std::collections::BTreeMap;

#[derive(Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct BencodableByteString(pub String);

#[derive(Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub enum Bencodable {
    ByteString(BencodableByteString),
    Integer(i32),
    List(Vec<Bencodable>),
    Dictionary(BTreeMap<BencodableByteString, Bencodable>),
}

pub fn bencode(b: &Bencodable) -> String {
    match b {
        Bencodable::ByteString(bs) => bs.0.len().to_string() + &":".to_string() + &bs.0,
        Bencodable::Integer(i) => "i".to_string() + &i.to_string() + &"e".to_string(),
        Bencodable::List(lb) => {
            "l".to_string()
                + &lb.iter().map(bencode).collect::<Vec<String>>().join("")
                + &"e".to_string()
        }
        Bencodable::Dictionary(m) => {
            "d".to_string()
                + &m.iter()
                    .map(|(k, v)| bencode(&Bencodable::ByteString(k.clone())) + &bencode(v))
                    .collect::<Vec<String>>()
                    .join("")
                + &"e".to_string()
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

pub struct DecodeError;

#[derive(Debug)]
pub struct BencodeParseError;

fn parse_byte_string(index: usize, bencoded_value: &str) -> Result<ParseResult, BencodeParseError> {
    let mut i = index;
    let mut length_string = String::new();
    let mut next_char = bencoded_value.chars().nth(i).ok_or(BencodeParseError)?;
    while next_char != ':' {
        i += 1;
        length_string.push(next_char);
        next_char = bencoded_value.chars().nth(i).ok_or(BencodeParseError)?;
    }
    let length = length_string.parse::<usize>().unwrap();
    Ok(ParseResult::from((
        i + 1 + length,
        Bencodable::ByteString(BencodableByteString(
            (&bencoded_value[i + 1..i + 1 + length]).to_string(),
        )),
    )))
}

fn parse_integer(index: usize, bencoded_value: &str) -> Result<ParseResult, BencodeParseError> {
    let mut i = index;
    let mut integer_string = String::new();
    let mut next_char = bencoded_value.chars().nth(i).ok_or(BencodeParseError)?;
    while next_char != 'e' {
        i += 1;
        integer_string.push(next_char);
        next_char = bencoded_value.chars().nth(i).ok_or(BencodeParseError)?;
    }
    let integer = integer_string.parse::<i32>().unwrap();
    Ok(ParseResult::from((i, Bencodable::Integer(integer))))
}

fn parse_list(index: usize, bencoded_value: &str) -> Result<ParseResult, BencodeParseError> {
    let mut i = index;
    let mut bencodables = vec![];
    let mut next_char = bencoded_value.chars().nth(i).ok_or(BencodeParseError)?;
    while next_char != 'e' {
        let item = parse_bencoded_value(i, bencoded_value)?;
        bencodables.push(item.bencodable);
        i = item.index;
        next_char = bencoded_value.chars().nth(i).ok_or(BencodeParseError)?;
    }
    Ok(ParseResult::from((i, Bencodable::List(bencodables))))
}

fn parse_dictionary(index: usize, bencoded_value: &str) -> Result<ParseResult, BencodeParseError> {
    let mut i = index;
    let mut bencodables = BTreeMap::new();
    let mut next_char = bencoded_value.chars().nth(i).ok_or(BencodeParseError)?;
    while next_char != 'e' {
        let byte_string_key =
            parse_bencoded_value(i, bencoded_value).and_then(|pr| match pr.bencodable {
                Bencodable::ByteString(bs) => Ok((pr.index, bs.0)),
                _ => Err(BencodeParseError),
            })?;
        let bencodable = parse_bencoded_value(byte_string_key.0, bencoded_value)?;
        bencodables.insert(
            BencodableByteString(byte_string_key.1),
            bencodable.bencodable,
        );
        i = bencodable.index;
        next_char = bencoded_value.chars().nth(i).ok_or(BencodeParseError)?;
    }
    Ok(ParseResult::from((i, Bencodable::Dictionary(bencodables))))
}

pub fn parse_bencoded_value(
    index: usize,
    bencoded_value: &str,
) -> Result<ParseResult, BencodeParseError> {
    let i = index;
    let first_byte = bencoded_value.chars().nth(i);
    first_byte.ok_or(BencodeParseError).and_then(|b| {
        if b.is_ascii_digit() {
            parse_byte_string(i, bencoded_value)
        } else if b == 'i' {
            parse_integer(i + 1, bencoded_value)
        } else if b == 'l' {
            parse_list(i + 1, bencoded_value)
        } else if b == 'd' {
            parse_dictionary(i + 1, bencoded_value)
        } else {
            Err(BencodeParseError)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }

    #[test]
    fn it_encodes_simple_dictionaries() {
        let mut examples = BTreeMap::new();
        examples.insert(
            BencodableByteString("Gedalia".to_string()),
            Bencodable::ByteString(BencodableByteString("Gedalia".to_string())),
        );
        examples.insert(
            BencodableByteString("a".to_string()),
            Bencodable::Integer(1),
        );
        assert_eq!(
            bencode(&Bencodable::Dictionary(examples)),
            "d7:Gedalia7:Gedalia1:ai1ee"
        );
    }

    #[test]
    fn it_encodes_integers() {
        assert_eq!(bencode(&Bencodable::Integer(-311)), "i-311e");
    }

    #[test]
    fn it_encodes_byte_strings() {
        assert_eq!(
            bencode(&Bencodable::ByteString(BencodableByteString(
                "Gedalia".to_string()
            ))),
            "7:Gedalia"
        );
    }

    #[test]
    fn it_encodes_long_byte_strings() {
        assert_eq!(
            bencode(&Bencodable::ByteString(BencodableByteString(
                "GedaliaGedalia".to_string()
            ))),
            "14:GedaliaGedalia"
        );
    }

    #[test]
    fn it_encodes_lists() {
        assert_eq!(
            "l4:spam4:eggsi-341ee",
            bencode(&Bencodable::List(vec!(
                Bencodable::ByteString(BencodableByteString(String::from("spam"))),
                Bencodable::ByteString(BencodableByteString(String::from("eggs"))),
                Bencodable::Integer(-341)
            )))
        );
    }

    #[test]
    fn it_encodes_more_complex_dictionaries() {
        let mut examples = BTreeMap::new();
        examples.insert(
            BencodableByteString("publisher".to_string()),
            Bencodable::ByteString(BencodableByteString("bob".to_string())),
        );
        examples.insert(
            BencodableByteString("publisher-webpage".to_string()),
            Bencodable::ByteString(BencodableByteString("www.example.com".to_string())),
        );
        examples.insert(
            BencodableByteString("publisher.location".to_string()),
            Bencodable::ByteString(BencodableByteString("home".to_string())),
        );
        assert_eq!(
            "d9:publisher3:bob17:publisher-webpage15:www.example.com18:publisher.location4:homee",
            bencode(&Bencodable::Dictionary(examples))
        );
    }

    #[test]
    fn it_decodes_byte_strings() {
        assert_eq!(
            parse_bencoded_value(0, "4:spam").unwrap().bencodable,
            Bencodable::ByteString(BencodableByteString(String::from("spam")))
        );
    }

    #[test]
    fn it_decodes_long_byte_strings() {
        assert_eq!(
            parse_bencoded_value(0, "14:GedaliaGedalia")
                .unwrap()
                .bencodable,
            Bencodable::ByteString(BencodableByteString(String::from("GedaliaGedalia")))
        );
    }

    #[test]
    fn it_decodes_lists() {
        assert_eq!(
            parse_bencoded_value(0, "i-3e").unwrap().bencodable,
            Bencodable::Integer(-3)
        );
    }

    #[test]
    fn it_decodes_integers() {
        assert_eq!(
            parse_bencoded_value(0, "i-341e").unwrap().bencodable,
            Bencodable::Integer(-341)
        );
    }

    #[test]
    fn it_decodes_small_integers() {
        assert_eq!(
            parse_bencoded_value(0, "i3e").unwrap().bencodable,
            Bencodable::Integer(3)
        );
    }

    #[test]
    fn it_decodes_heterogenous_lists() {
        assert_eq!(
            parse_bencoded_value(0, "l4:spam4:eggsi-341ee")
                .unwrap()
                .bencodable,
            Bencodable::List(vec!(
                Bencodable::ByteString(BencodableByteString(String::from("spam"))),
                Bencodable::ByteString(BencodableByteString(String::from("eggs"))),
                Bencodable::Integer(-341)
            ))
        );
    }

    #[test]
    fn it_decodes_dictionaries() {
        let mut examples = BTreeMap::new();
        examples.insert(
            BencodableByteString("Gedalia".to_string()),
            Bencodable::ByteString(BencodableByteString("Gedalia".to_string())),
        );
        examples.insert(
            BencodableByteString("a".to_string()),
            Bencodable::Integer(1),
        );
        assert_eq!(
            parse_bencoded_value(0, "d7:Gedalia7:Gedalia1:ai1ee")
                .unwrap()
                .bencodable,
            Bencodable::Dictionary(examples)
        );
    }

    #[test]
    fn it_decodes_dictionaries_with_embedded_list_values() {
        let mut examples = BTreeMap::new();
        examples.insert(
            BencodableByteString("spam".to_string()),
            Bencodable::List(vec![
                Bencodable::ByteString(BencodableByteString("a".to_string())),
                Bencodable::ByteString(BencodableByteString("b".to_string())),
            ]),
        );
        assert_eq!(
            parse_bencoded_value(0, "d4:spaml1:a1:bee")
                .unwrap()
                .bencodable,
            Bencodable::Dictionary(examples)
        );
    }

    #[test]
    fn it_decodes_complex_dictionaries() {
        let mut examples = BTreeMap::new();
        examples.insert(
            BencodableByteString("publisher".to_string()),
            Bencodable::ByteString(BencodableByteString("bob".to_string())),
        );
        examples.insert(
            BencodableByteString("publisher-webpage".to_string()),
            Bencodable::ByteString(BencodableByteString("www.example.com".to_string())),
        );
        examples.insert(
            BencodableByteString("publisher.location".to_string()),
            Bencodable::ByteString(BencodableByteString("home".to_string())),
        );
        assert_eq!(
            parse_bencoded_value(
                0,
                "d9:publisher3:bob17:publisher-webpage15:www.example.com18:publisher.location4:homee"
            )
            .unwrap()
            .bencodable,
            Bencodable::Dictionary(examples)
        );
    }

    #[test]
    fn it_decodes_empty_dictionaries() {
        assert_eq!(
            parse_bencoded_value(0, "de").unwrap().bencodable,
            Bencodable::Dictionary(BTreeMap::new())
        );
    }

    #[test]
    fn it_decodes_empty_lists() {
        assert_eq!(
            parse_bencoded_value(0, "le").unwrap().bencodable,
            Bencodable::List(Vec::new())
        );
    }
}
