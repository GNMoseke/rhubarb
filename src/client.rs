use crate::log::*;
use crate::util::*;
use base64ct::{Base64, Encoding};
use sha1::{Digest, Sha1};
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    net::{Shutdown, TcpStream},
};

pub(crate) const HARDCODED_HANDSHAKE: &str = "GET /ws HTTP/1.1
Host: 127.0.0.1:4024
Upgrade: websocket
Connection: Upgrade
Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==
Sec-WebSocket-Protocol: rhubarb
Sec-WebSocket-Version: 13
";

pub(crate) struct WebSocketClient<S: Stream> {
    stream: S,
}

impl Clone for WebSocketClient<TcpStream> {
    fn clone(&self) -> Self {
        Self {
            stream: self.stream.try_clone().expect("cloning tcp stream"),
        }
    }
}

impl WebSocketClient<TcpStream> {
    pub(crate) fn create(bind_addr: &str) -> std::io::Result<WebSocketClient<TcpStream>> {
        let _stream = TcpStream::connect(bind_addr)?;
        Ok(WebSocketClient { stream: _stream })
    }

    pub(crate) fn send(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.stream.write_all(data)?;
        Ok(())
    }

    pub(crate) fn recv(self) -> std::io::Result<()> {
        let mut reader = BufReader::new(self.stream);
        loop {
            let recv: Vec<u8> = reader.fill_buf()?.to_vec();
            reader.consume(recv.len());
            let message = String::from_utf8(recv).unwrap();
            if !message.is_empty() {
                print!("{}", message);
            }
        }
    }

    pub(crate) fn perform_handshake(&mut self, path: String) -> std::io::Result<()> {
        self.log(String::from("Performing Handshake"), LogLevel::Info);
        let (request, key) = self.create_handshake_http_request(path);
        self.send(request.as_bytes())?;

        // wait for response
        let mut reader = BufReader::new(self.stream.try_clone()?);
        let recv: Vec<u8> = reader.fill_buf()?.to_vec();
        reader.consume(recv.len());

        let response = String::from_utf8(recv).map_err(|_| {
            self.stream
                .shutdown(Shutdown::Both)
                .expect("Shutdown succeeded");
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Failed to parse handshake as utf8",
            )
        })?;

        self.validate_server_handshake(response, key).map_err(|e| {
            self.stream
                .shutdown(Shutdown::Both)
                .expect("Shutdown succeeded");
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        })?;

        Ok(())
    }
}

// NOTE: per the RFC, there's a `connecting` state for clients attempting to connect to the same
// remote simultaneously. rhubarb in its current state doesn't allow multiple client connections
// from one process anyway, so I'm ignoring this for now.
impl<S: Stream> WebSocketClient<S> {
    fn validate_server_handshake(
        &self,
        server_response: String,
        key: String,
    ) -> Result<(), String> {
        self.log(
            format!("Validating client handshake\n{}", server_response),
            LogLevel::Debug,
        );

        let mut components = server_response.trim().split('\n');
        // pop the http version & response code
        let http_response = match components.next() {
            Some(r) => r,
            None => return Err(String::from("Handshake is not a valid HTTP response")),
        };

        // validation 1 - must be 101 switching protocols
        // for rhubarb, I ignore anything else and just error
        let mut response_components = http_response.split_whitespace();
        response_components.next();
        match response_components.next() {
            Some("101") => {}
            Some(resp_code) => {
                return Err(String::from(format!("Invalid response code {}", resp_code)))
            }
            None => return Err(String::from("Missing response code")),
        };

        let headers = components
            .filter_map(|header| header.split_once(':'))
            .map(|(header_name, val)| (header_name.trim().to_lowercase(), val.trim()))
            .collect::<HashMap<_, _>>();

        // validation 2 - must include "upgrade: websocket" header
        match headers.get("upgrade") {
            Some(ug) if ug.to_lowercase() == "websocket" => {}
            Some(_) => return Err(String::from("Requested Upgrade was not 'websocket'")),
            None => return Err(String::from("Handshake missing Upgrade header")),
        };

        // validation 3 - must include "connection: upgrade" header
        match headers.get("connection") {
            Some(conn) if conn.to_lowercase() == "upgrade" => {}
            Some(_) => return Err(String::from("Requested Connection was not 'upgrade'")),
            None => return Err(String::from("Handshake missing Connection header")),
        };

        // validation 4 - key validation
        let accept_key = match headers.get("sec-websocket-accept") {
            Some(h) => h.trim().to_string(),
            None => {
                return Err(String::from(
                    "Handshake missing Sec-WebSocket-Accept header",
                ))
            }
        };

        let hash = Sha1::digest((key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").as_bytes());
        let expected_key = Base64::encode_string(&hash);

        if accept_key != expected_key {
            return Err(String::from("Server key invalid"));
        }

        Ok(())
    }

    fn create_handshake_http_request(&self, path: String) -> (String, String) {
        let mut nonce = [0u8; 16];
        rand::fill(&mut nonce);
        let key = Base64::encode_string(&nonce);
        (
            format!(
                "GET {path} HTTP/1.1\n\
            Host: {}\n\
            Upgrade: websocket\n\
            Connection: Upgrade\n\
            Sec-WebSocket-Key: {}\n\
            Sec-WebSocket-Protocol: rhubarb\n\
            Sec-WebSocket-Version: 13\n
            ",
                self.stream
                    .peer_addr()
                    .expect("peer address found")
                    .to_string(),
                key
            ),
            key,
        )
    }

    fn log(&self, msg: String, level: LogLevel) {
        // NOTE: this expect is half-reasonable since if we can't get a peer addr how are we
        // connected, but it should probably be handled more gracefully
        log(
            format!(
                "{} - {msg}",
                self.stream.peer_addr().expect("peer address found")
            ),
            level,
        );
    }
}

#[cfg(test)]
mod tests {}
