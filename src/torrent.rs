use crate::BitField;
use std::io::Write;

use std::time::Instant;

pub trait PiecedContent {
    fn number_of_pieces(&self) -> u32;
    fn piece_length(&self) -> u32;
    fn total_length(&self) -> u32;
    fn name(&self) -> String;
}

#[derive(Debug)]
pub struct Torrent {
    total_blocks: u32,
    pieces: Vec<Piece>,
    piece_length: u32,
    num_blocks_per_piece: u32,
    file_name: String,
}

#[derive(Debug)]
struct Piece {
    index: u32,
    blocks: Vec<Block>,
    length: u32,
}

#[derive(Debug)]
pub struct Block {
    data: Option<Vec<u8>>,
    state: BlockState,
    pub index: u32,
    pub piece_index: u32,
    pub offset: u32,
    pub length: u32,
    last_request: Option<Instant>,
}

#[derive(Debug)]
enum BlockState {
    NotRequested,
    Requested,
    Done,
}

const FIXED_BLOCK_SIZE: u32 = 16384;

impl Torrent {
    pub fn new(meta_info_file: &dyn PiecedContent) -> Self {
        let number_of_pieces = meta_info_file.number_of_pieces();
        let piece_length = meta_info_file.piece_length();
        let total_length = meta_info_file.total_length();

        let number_of_blocks =
            (piece_length / FIXED_BLOCK_SIZE) + !!(piece_length % FIXED_BLOCK_SIZE);

        let mut pieces: Vec<Piece> = (0..(number_of_pieces - 1))
            .map(|index| {
                let blocks: Vec<Block> = (0..number_of_blocks)
                    .map(|block_index| Block {
                        state: BlockState::NotRequested,
                        index: block_index,
                        piece_index: index,
                        offset: FIXED_BLOCK_SIZE * block_index,
                        length: FIXED_BLOCK_SIZE,
                        data: None,
                        last_request: None,
                    })
                    .collect();
                Piece {
                    index,
                    blocks,
                    length: piece_length,
                }
            })
            .collect();

        let last_piece_length = total_length % piece_length;
        let last_piece_block_count =
            (last_piece_length as f32 / FIXED_BLOCK_SIZE as f32).ceil() as u32;
        let last_piece_index = (total_length as f32 / piece_length as f32).floor() as u32;

        let last_blocks = (0..last_piece_block_count)
            .map(|block_index| Block {
                state: BlockState::NotRequested,
                index: block_index,
                piece_index: last_piece_index,
                offset: FIXED_BLOCK_SIZE * block_index,
                length: FIXED_BLOCK_SIZE,
                data: None,
                last_request: None,
            })
            .collect();

        pieces.push(Piece {
            index: last_piece_index,
            blocks: last_blocks,
            length: last_piece_length,
        });

        let total_blocks = ((number_of_pieces - 1) * number_of_blocks) + last_piece_block_count;

        Torrent {
            total_blocks,
            piece_length,
            pieces,
            num_blocks_per_piece: number_of_blocks,
            file_name: meta_info_file.name(),
        }
    }

    // Would an iterator approach speed things up? Can this be opimized in general?
    pub fn get_next_block(&mut self, bitfield: &BitField) -> Option<(u32, u32, u32)> {
        let (progress, _, _, _) = self.progress();
        for p in self.pieces.iter_mut() {
            // is this piece one that is present in the bitfield?
            if bitfield.is_set(p.index as usize).is_ok() {
                for b in p.blocks.iter_mut() {
                    match b.state {
                        BlockState::Done => continue,
                        BlockState::Requested => {
                            if progress > 95.0 {
                                return Some((b.piece_index, b.offset, b.length));
                            }
                            let now = Instant::now();
                            let last = b.last_request.unwrap();
                            if now - last > std::time::Duration::from_secs(15) {
                                // println!("re-requesting block... diff: {:?}, now: {:?}, last: {:?}", now - last, now, b.last_request);
                                b.last_request = Some(now);
                                return Some((b.piece_index, b.offset, b.length));
                            } else {
                                continue;
                            }
                        }
                        BlockState::NotRequested => {
                            b.state = BlockState::Requested;
                            b.last_request = Some(Instant::now());
                            return Some((b.piece_index, b.offset, b.length));
                        }
                    }
                }
            } else {
                println!("connection didn't have bit {:?} set", p.index as usize);
                continue;
            }
        }
        None
    }

    pub fn fill_block(&mut self, block: (u32, u32, &[u8])) {
        let (index, offset, data) = block;
        let piece = self.pieces.get_mut(index as usize);
        let block_index = offset / FIXED_BLOCK_SIZE;

        match piece {
            None => {}
            Some(piece) => {
                let b = piece.blocks.get_mut(block_index as usize);
                match b {
                    None => {}
                    Some(b) => {
                        b.state = BlockState::Done;
                        b.data = Some(data.to_vec());
                    }
                }
            }
        }
    }

    pub fn to_file(&self) -> std::fs::File {
        let file_name = &self.file_name;
        let mut file = std::fs::File::create(file_name).unwrap();
        for p in &self.pieces {
            for b in &p.blocks {
                let bytes = b.data.as_ref();
                match bytes {
                    Some(b) => {
                        file.write_all(&b).unwrap();
                    }
                    None => {
                        println!("missing block {:?} of piece {:?}", b.offset, b.piece_index)
                    }
                }
            }
        }
        file
    }

    pub fn are_we_done_yet(&self) -> bool {
        let mut completed = 0;
        for p in &self.pieces {
            for b in &p.blocks {
                match b.state {
                    BlockState::Done => completed += 1,
                    BlockState::Requested => return false,
                    BlockState::NotRequested => return false,
                }
            }
        }
        completed == self.total_blocks
    }

    pub fn progress(&self) -> (f32, u32, u32, u32) {
        let mut completed = 0;
        let mut requested = 0;
        let mut not_requested = 0;
        for p in &self.pieces {
            for b in &p.blocks {
                match b.state {
                    BlockState::Done => completed += 1,
                    BlockState::Requested => requested += 1,
                    BlockState::NotRequested => not_requested += 1,
                }
            }
        }
        let percent_complete = completed as f32 / self.total_blocks as f32;
        (percent_complete, completed, requested, not_requested)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeMetaInfo;
    impl PiecedContent for FakeMetaInfo {
        fn number_of_pieces(&self) -> u32 {
            (170835968f32 / 131072f32).ceil() as u32
        }
        fn piece_length(&self) -> u32 {
            131072
        }
        fn name(&self) -> String {
            String::from("Charlie_Chaplin_Mabels_Strange_Predicament.avi")
        }
        fn total_length(&self) -> u32 {
            170835968
        }
    }

    #[test]
    fn gets_the_next_block_correctly() {
        let pieced_content = &FakeMetaInfo {};
        let t = Torrent::new(pieced_content);

        assert_eq!(1304, t.pieces.len());

        let other = t.pieces.first().unwrap();
        assert_eq!(8, other.blocks.len());

        let last = t.pieces.last().unwrap();
        let expected_last_length = pieced_content.total_length() % (t.piece_length);
        assert_eq!(last.length, expected_last_length);

        assert_eq!(3, last.blocks.len());

        assert_eq!(10427, t.total_blocks);
    }
}
