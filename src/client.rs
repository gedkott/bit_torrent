use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

use crate::messages::{Message, MessageParseError};

#[derive(Debug)]
pub enum HandshakeError {
    TcpWrite,
    TcpReturnHandshakeRead(std::io::Error, [u8; 68]),
}

#[derive(Debug)]
pub enum InterestedError {
    TcpWrite,
}

pub trait PeerStream {
    fn handshake(&mut self, info_hash: &[u8]) -> Result<(), HandshakeError>;
    fn interested(&mut self) -> Result<(), InterestedError>;
}

pub trait PeerStreamT: PeerStream + std::fmt::Debug + Send + Sync {}

pub type MessageReceiver = Receiver<(
    Arc<Mutex<dyn PeerStreamT>>,
    Result<Message, MessageParseError>,
)>;

pub type MessageSender = std::sync::mpsc::Sender<(
    std::sync::Arc<std::sync::Mutex<dyn crate::PeerStreamT>>,
    std::result::Result<Message, MessageParseError>,
)>;

pub type Threads = Vec<(std::thread::JoinHandle<()>, Arc<Mutex<dyn PeerStreamT>>)>;

pub struct Readers {
    pub receiver: MessageReceiver,
    pub threads: Threads,
}

pub trait PeerClient {
    fn connect(peers: &[crate::tracker::Peer], info_hash: &[u8]) -> Self;
    fn listen(self) -> Readers;
}
