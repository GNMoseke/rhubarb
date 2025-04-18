use std::{
    io::Write,
    net::TcpStream,
};

pub(crate) const HARDCODED_HANDSHAKE: &[u8] = b"
GET /ws HTTP/1.1
Host: localhost
Upgrade: websocket
Connection: Upgrade
Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==
Origin: localhost
Sec-WebSocket-Protocol: rhubarb
Sec-WebSocket-Version: 13
";

pub(crate) struct WebSocketClient {
    _stream: TcpStream,
}

impl WebSocketClient {
    pub(crate) fn create(bind_addr: &str) -> std::io::Result<WebSocketClient> {
        let _stream = TcpStream::connect(bind_addr)?;
        Ok(WebSocketClient { _stream })
    }

    pub(crate) fn send(&mut self, data: &[u8]) -> std::io::Result<()> {
        self._stream.write(data)?;
        Ok(())
    }
}
