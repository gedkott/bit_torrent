// &[SocketAddr] -> { connect to each socketAddr -> ... do protocol... -> maintain pool of connections -> }

use std::net::{TcpStream, SocketAddr};

pub struct PeerTcpClient {
    connections: Box<dyn std::iter::Iterator<Item=std::io::Result<TcpStream>>>
}

impl<'a> PeerTcpClient {
    pub fn connect<'b>(peer_socket_addrs: &'a [SocketAddr]) -> Self {
        let connections = peer_socket_addrs.clone().iter().map(|sa| { TcpStream::connect(sa) } );
        PeerTcpClient {
            connections: Box::new(connections)
        }
    }
}

