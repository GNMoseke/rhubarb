use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    net::{Shutdown, TcpListener, TcpStream},
};

pub(crate) struct WebSocketServer {
    _listener: TcpListener,
}


impl WebSocketServer {
    pub(crate) fn create(bind_addr: &str) -> std::io::Result<WebSocketServer> {
        let _listener = TcpListener::bind(bind_addr)?;
        Ok(WebSocketServer { _listener })
    }

    pub(crate) fn listen(self) -> std::io::Result<()> {
        for stream in self._listener.incoming() {
            if let Ok(stream) = stream {
                // TODO: dispatch each client to its own thread so that one bad handshake doesn't take
                // down the server
                self.handle_client(stream)?;
            }
        }
        Ok(())
    }

    pub(crate) fn handle_client(&self, mut stream: TcpStream) -> std::io::Result<()> {
        let mut reader = BufReader::new(stream.try_clone()?);
        let recv: Vec<u8> = reader.fill_buf()?.to_vec();
        reader.consume(recv.len());

        // need to first handle the handshake, then start processing data
        let handshake = String::from_utf8(recv).map_err(|_| {
            // TODO: handle a failed shutdown
            _ = stream.shutdown(Shutdown::Both);
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Failed to parse handshake as utf8",
            )
        })?;
        println!("{}", handshake);

        // echo back whatever we get from here on
        loop {
            let recv: Vec<u8> = reader.fill_buf()?.to_vec();
            reader.consume(recv.len());
            let message = String::from_utf8(recv).unwrap();
            if message.len() > 0 {
                println!("{}", message);
                _ = stream.write(message.as_bytes());
            }
        }
    }
}
