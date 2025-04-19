use std::net::TcpStream;

pub(crate) trait Stream {
    fn peer_addr(&self) -> std::io::Result<std::net::SocketAddr>;
}

impl Stream for TcpStream {
    fn peer_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        self.peer_addr()
    }
}
