use base64ct::{Base64, Encoding};
use sha1::{Digest, Sha1};
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    net::{Shutdown, TcpListener, TcpStream},
};

use crate::client;

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

    /// Returns a result with either a valid value for Sec-WebSocket-Accept, or a string to be used
    /// in a 400 bad request
    fn validate_handshake(client_handshake: String) -> Result<String, String> {
        let mut components = client_handshake.split('\n');
        // pop the method + path + http version
        let http_request = components.next();

        // validation 1 - must be a GET request, with a valid Request-URI with HTTP/1.1 or higher

        // TODO: I'm just chucking the rest of the headers here, but I could return them as part
        // of a tuple or struct or something, then pass back to a closure on the `handle_client`
        // and `listen` funcs.
        // e.g. the api is something like:
        // WebSocketClient::create("...").listen(on_initial_conn: { request }, on_recv: { bytes })
        // ergonomics wise I could also register those callbacks using their own funcs
        // or both, both is good
        let headers = components
            .filter_map(|header| header.split_once(':'))
            .map(|(header_name, val)| (header_name.to_lowercase(), val))
            .collect::<HashMap<_, _>>();

        let mut key = match headers.get("sec-websocket-key") {
            Some(h) => h.trim().to_string(),
            None => return Err(String::from("Handshake missing Sec-WebSocket-key header")),
        };

        // validation 2 - must include a Host header matching server
        
        // validation 3 - must include "upgrade: websocket" header

        // validation 4 - must include "connection: upgrade" header

        // validation 6 - "sec-websocket-version: 13". Process before key to avoid the hash if we
        // can

        // validation 5 - key
        // This key must be exactly 24 characters (b64 on a 16 byte nonce), as per
        // https://www.rfc-editor.org/rfc/rfc6455#section-4.1

        // the magic UUID from https://www.rfc-editor.org/rfc/rfc6455#section-1.3
        key += "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

        // TODO: I would like to write a pure-rust version of this myself, but right now I'm cheating and
        // just calling into rustcrypto
        let hash = Sha1::digest(key.as_bytes());
        let base64_hash = Base64::encode_string(&hash);
        return Ok(base64_hash);
    }
}

#[test]
fn test_validate_handshake() {
    assert_eq!(
        WebSocketServer::validate_handshake(client::HARDCODED_HANDSHAKE.to_string()),
        Ok(String::from("s3pPLMBiTxaQ9kYGzzhZRbK+xOo="))
    );
    assert!(WebSocketServer::validate_handshake(String::from("")).is_err());
}
