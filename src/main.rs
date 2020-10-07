use std::collections::BTreeMap;

use bencoding::*;

fn main() {
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

    assert_eq!(
        parse_bencoded_value(0, "4:spam").unwrap().bencodable,
        Bencodable::ByteString(BencodableByteString(String::from("spam")))
    );
}
