use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::Write;
use std::time::Instant;

use crate::BitField;

pub trait PiecedContent {
    fn number_of_pieces(&self) -> u32;
    fn piece_length(&self) -> u32;
    fn total_length(&self) -> u32;
    fn name(&self) -> String;
}

#[derive(Debug)]
struct Piece {
    index: u32,
    blocks: VecDeque<Block>,
    length: u32,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Block {
    data: Option<Vec<u8>>,
    state: BlockState,
    offset: u32,
    last_request: Option<Instant>,
    piece_index: u32,
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum BlockState {
    NotRequested,
    Requested,
    Done,
}

const FIXED_BLOCK_SIZE: u32 = 16384;

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

    in_progress_blocks: Vec<Block>,
    completed_pieces: Vec<Vec<Option<Block>>>,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct PieceIndexOffsetLength(pub u32, pub u32, pub u32);

impl Torrent {
    pub fn new(pieced_content: &dyn PiecedContent) -> Self {
        let number_of_pieces = pieced_content.number_of_pieces();
        let piece_length = pieced_content.piece_length();
        let total_length = pieced_content.total_length();

        let number_of_blocks =
            (piece_length / FIXED_BLOCK_SIZE) + !!(piece_length % FIXED_BLOCK_SIZE);

        let mut pieces: Vec<Piece> = (0..(number_of_pieces - 1))
            .map(|index| {
                let blocks: VecDeque<Block> = (0..number_of_blocks)
                    .map(|block_index| Block {
                        state: BlockState::NotRequested,
                        offset: FIXED_BLOCK_SIZE * block_index,
                        data: None,
                        last_request: None,
                        piece_index: index,
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
                piece_index: (pieces.len()) as u32,
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
            in_progress_blocks: vec![],
            completed_pieces: (0..number_of_pieces)
                .map(|_pi| (0..number_of_blocks).map(|_bi| None).collect())
                .collect(),
        }
    }

    pub fn get_next_block(&mut self, bitfield: &BitField) -> Option<PieceIndexOffsetLength> {
        if self.in_progress_blocks.len() == 256 {
            // there are no more blocks for the requester to help with "right now"
            println!(
                "we are at capacity for new in progress blocks; current in progress: {:?}",
                self.in_progress_blocks
                    .iter()
                    .map(|block| { (block.piece_index, block.offset) })
            );
            return None;
        }

        let res: Option<(u32, &mut VecDeque<Block>)> = {
            let mut res = None;
            // O(total number of pieces); always pulls pieces and blocks based on exact order of index of piece from 0 to total number of pieces
            for piece in self.pieces.iter_mut() {
                let piece_index = piece.index;

                // relatively cheap; should not panic!!!
                match bitfield.is_set(piece_index as usize).unwrap() {
                    true => {
                        let blocks_to_request_queue = &mut piece.blocks;
                        res = Some((piece_index, blocks_to_request_queue));
                        break;
                    }
                    false => continue,
                }
            }
            res
        };

        match res {
            Some((piece_index, blocks_to_request_queue)) => {
                // we can give them any block in p.index's block queue
                let mut next_block = blocks_to_request_queue.pop_front().expect("tried to get a block from a piece's queue, but it was empty even when piece wasn't marked as done"); // It shouldn't be empty since piece was not complete...
                let offset = next_block.offset;
                next_block.state = BlockState::Requested;
                next_block.last_request = Some(Instant::now());
                self.requested_blocks += 1;

                self.in_progress_blocks.push(next_block);

                if blocks_to_request_queue.is_empty() {
                    let index = self
                        .pieces
                        .iter()
                        .position(|piece| piece.index == piece_index)
                        .expect(
                            "tried to remove a completed piece from the pieces field and failed",
                        );
                    self.pieces.swap_remove(index);
                }

                Some(PieceIndexOffsetLength(
                    piece_index,
                    offset,
                    FIXED_BLOCK_SIZE,
                ))
            }
            None => None,
        }
    }

    pub fn fill_block(&mut self, block: (u32, u32, &[u8])) {
        let (piece_index, offset, data) = block;
        let block_index = offset / FIXED_BLOCK_SIZE;

        let index = self
            .in_progress_blocks
            .iter()
            .position(|block| block.piece_index == piece_index && block.offset == offset)
            .unwrap_or_else(|| panic!("we should never be trying to fill a piece index and block offset: {:?} that wasn't in the in_progress_blocks field: {:?}", (piece_index, offset), self.in_progress_blocks
                .iter()
                .map(|block| {
                    (block.piece_index, block.offset)
                })
            ));

        let b = &mut self.in_progress_blocks[index];

        if b.state != BlockState::Done {
            b.state = BlockState::Done;
            b.data = Some(data.to_vec());
            self.completed_blocks += 1;
            self.percent_complete = self.completed_blocks as f32 / self.total_blocks as f32;
            self.completed_pieces[piece_index as usize][block_index as usize] =
                Some(self.in_progress_blocks.swap_remove(index));
        } else {
            self.repeated_blocks
                .entry((piece_index, offset))
                .and_modify(|v| *v += 1)
                .or_insert(1);
        }
    }

    pub fn to_file(&self) -> File {
        let file_name = &self.file_name;
        let mut file = File::create(file_name).unwrap();
        for (piece_index, list_of_filled_blocks) in self.completed_pieces.iter().enumerate() {
            for (block_index, block) in list_of_filled_blocks.iter().enumerate() {
                match block {
                    Some(b) => {
                        let bytes = b.data.as_ref();
                        match bytes {
                            Some(b) => {
                                file.write_all(b).unwrap();
                            }
                            None => {
                                // println!(
                                //     "missing block {:?} of piece {:?}",
                                //     b.offset, b.piece_index
                                // )
                            }
                        }
                    }
                    None => {
                        // println!(
                        //     "missing block index {:?} of piece {:?}",
                        //     block_index, piece_index
                        // )
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
        let mut t = Torrent::new(pieced_content);

        assert_eq!(1304, t.pieces.len());

        let other = t.pieces.first().unwrap();
        assert_eq!(8, other.blocks.len());

        let last = t.pieces.last().unwrap();
        let expected_last_length = 49152;
        assert_eq!(last.length, expected_last_length);

        assert_eq!(3, last.blocks.len());

        assert_eq!(10427, t.total_blocks);

        let bf = &BitField::from(vec![255; 1304]);

        for i in 0..8 {
            let next_block = t.get_next_block(bf);
            assert_eq!(
                Some(PieceIndexOffsetLength(
                    0,
                    FIXED_BLOCK_SIZE * i,
                    FIXED_BLOCK_SIZE
                )),
                next_block
            );
            t.fill_block((0, FIXED_BLOCK_SIZE * i, &[]));
        }

        for i in 0..3 {
            let next_block = t.get_next_block(bf);
            assert_eq!(
                Some(PieceIndexOffsetLength(
                    1303,
                    FIXED_BLOCK_SIZE * i,
                    FIXED_BLOCK_SIZE
                )),
                next_block
            );
            t.fill_block((1303, FIXED_BLOCK_SIZE * i, &[]));
        }

        for i in 0..8 {
            let next_block = t.get_next_block(bf);
            assert_eq!(
                Some(PieceIndexOffsetLength(
                    1302,
                    FIXED_BLOCK_SIZE * i,
                    FIXED_BLOCK_SIZE
                )),
                next_block
            );
            t.fill_block((1302, FIXED_BLOCK_SIZE * i, &[]));
        }
    }
}
