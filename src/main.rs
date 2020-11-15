use std::collections::BTreeMap;
use std::fs::File;
use std::io::prelude::*;

mod bencode;
use bencode::*;

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

    let mut f = File::open("big-buck-bunny.torrent").unwrap();
    let mut bytes = Vec::new();
    f.read_to_end(&mut bytes).unwrap();
    let decoded_original = bdecode(bytes.as_slice()).unwrap();

    File::create("myfile.torrent")
        .and_then(|mut f| f.write_all(bencode(&decoded_original).unwrap().as_slice()))
        .ok();

    let mut f = File::open("myfile.torrent").unwrap();
    let mut bytes = Vec::new();
    f.read_to_end(&mut bytes).unwrap();

    let decoded_from_new_file_written_with_encoded_original = bdecode(bytes.as_slice()).unwrap();

    assert_eq!(
        decoded_original,
        decoded_from_new_file_written_with_encoded_original
    );
}
