use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::time::{Duration, Instant};

use crate::BitField;

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
    pub total_pieces: u32,
    file_name: String,
    completed_blocks: u32,
    requested_blocks: u32,
    pub percent_complete: f32,
    pub repeated_blocks: HashMap<(u32, u32), u32>,
}

#[derive(Debug)]
struct Piece {
    index: u32,
    blocks: Vec<Block>,
    length: u32,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Block {
    data: Option<Vec<u8>>,
    state: BlockState,
    offset: u32,
    last_request: Option<Instant>,
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum BlockState {
    NotRequested,
    Requested,
    Done,
}

const FIXED_BLOCK_SIZE: u32 = 16384;
const END_GAME_PROGRESS_THRESHOLD: f32 = 92.5;
const REQUEST_WAIT_TIME: Duration = Duration::from_secs(15);

impl Torrent {
    pub fn new(pieced_content: &dyn PiecedContent) -> Self {
        let number_of_pieces = pieced_content.number_of_pieces();
        let piece_length = pieced_content.piece_length();
        let total_length = pieced_content.total_length();

        let number_of_blocks =
            (piece_length / FIXED_BLOCK_SIZE) + !!(piece_length % FIXED_BLOCK_SIZE);

        let mut pieces: Vec<Piece> = (0..(number_of_pieces - 1))
            .map(|index| {
                let blocks: Vec<Block> = (0..number_of_blocks)
                    .map(|block_index| Block {
                        state: BlockState::NotRequested,
                        offset: FIXED_BLOCK_SIZE * block_index,
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
                offset: FIXED_BLOCK_SIZE * block_index,
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
            pieces,
            total_pieces: number_of_pieces,
            file_name: pieced_content.name(),
            completed_blocks: 0,
            requested_blocks: 0,
            percent_complete: 0.0,
            repeated_blocks: HashMap::new(),
        }
    }

    pub fn get_next_block(&mut self, bitfield: &BitField) -> Option<(u32, u32, u32)> {
        for p in self.pieces.iter_mut() {
            // is this piece one that is present in the bitfield?
            if bitfield.is_set(p.index as usize).is_ok() {
                for b in p.blocks.iter_mut() {
                    match b.state {
                        BlockState::Done => continue,
                        BlockState::Requested => {
                            if self.percent_complete > END_GAME_PROGRESS_THRESHOLD {
                                return Some((p.index, b.offset, FIXED_BLOCK_SIZE));
                            }
                            let now = Instant::now();
                            let last = b.last_request.unwrap();
                            if now - last > REQUEST_WAIT_TIME {
                                b.last_request = Some(now);
                                return Some((p.index, b.offset, FIXED_BLOCK_SIZE));
                            } else {
                                continue;
                            }
                        }
                        BlockState::NotRequested => {
                            b.state = BlockState::Requested;
                            b.last_request = Some(Instant::now());
                            self.requested_blocks += 1;
                            return Some((p.index, b.offset, FIXED_BLOCK_SIZE));
                        }
                    }
                }
            } else {
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
                        if b.state != BlockState::Done {
                            b.state = BlockState::Done;
                            b.data = Some(data.to_vec());
                            self.completed_blocks += 1;
                            self.percent_complete =
                                self.completed_blocks as f32 / self.total_blocks as f32;
                        } else {
                            self.repeated_blocks
                                .entry((piece.index, b.offset))
                                .and_modify(|v| *v += 1)
                                .or_insert(1);
                        }
                    }
                }
            }
        }
    }

    pub fn to_file(&self) -> File {
        let file_name = &self.file_name;
        let mut file = File::create(file_name).unwrap();
        for p in &self.pieces {
            for b in &p.blocks {
                let bytes = b.data.as_ref();
                match bytes {
                    Some(b) => {
                        file.write_all(b).unwrap();
                    }
                    None => {
                        println!("missing block {:?} of piece {:?}", b.offset, p.index)
                    }
                }
            }
        }
        file
    }

    pub fn are_we_done_yet(&self) -> bool {
        self.completed_blocks == self.total_blocks
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
        let expected_last_length = 49152;
        assert_eq!(last.length, expected_last_length);

        assert_eq!(3, last.blocks.len());

        assert_eq!(10427, t.total_blocks);
    }
}
