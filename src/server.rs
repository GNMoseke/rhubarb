use crate::log;
use base64ct::{Base64, Encoding};
use sha1::{Digest, Sha1};
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    net::{Shutdown, TcpListener, TcpStream},
};

pub(crate) struct WebSocketServer {
    _listener: TcpListener,
}

struct ServerHandle<S: Stream> {
    stream: S,
}

pub(crate) trait Stream {
    fn peer_addr(&self) -> std::io::Result<std::net::SocketAddr>;
}
impl Stream for TcpStream {
    fn peer_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        self.peer_addr()
    }
}

impl WebSocketServer {
    pub(crate) fn create(bind_addr: &str) -> std::io::Result<WebSocketServer> {
        let _listener = TcpListener::bind(bind_addr)?;
        Ok(WebSocketServer { _listener })
    }

    pub(crate) fn listen(self) -> std::io::Result<()> {
        for stream in self._listener.incoming().flatten() {
            std::thread::spawn(|| {
                let mut handle = ServerHandle::<TcpStream> { stream };
                handle.handle_client()
            });
        }
        Ok(())
    }
}

impl ServerHandle<TcpStream> {
    pub(crate) fn handle_client(&mut self) -> std::io::Result<()> {
        self.log(String::from("New Client Connected"), log::LogLevel::Info);
        let mut reader = BufReader::new(self.stream.try_clone()?);
        let recv: Vec<u8> = reader.fill_buf()?.to_vec();
        reader.consume(recv.len());

        // need to first handle the handshake, then start processing data
        let handshake = String::from_utf8(recv).map_err(|_| {
            self.stream
                .shutdown(Shutdown::Both)
                .expect("Shutdown failed");
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Failed to parse handshake as utf8",
            )
        })?;

        match self.validate_handshake(
            handshake,
            self.stream
                .local_addr()
                .expect("no local address found")
                .to_string(),
        ) {
            // TODO: handle other HTTP protocol values, Sec-WebSocket-Protocol,
            // Sec-WebSocket-Extensions, and any additional headers
            Ok(key) => {
                let response = format!(
                    "HTTP/1.1 101 Switching Protocols
                    Upgrade: websocket
                    Connection: Upgrade
                    Sec-WebSocket-Accept: {key}"
                );
                self.stream.write_all(response.as_bytes())?;
            }
            Err(msg) => {
                self.log(
                    format!("Handshake failed - {}", msg),
                    log::LogLevel::Warning,
                );
                let response = format!("HTTP/1.1 400 Bad Request\r\n\r\n{msg}");
                self.stream.write_all(response.as_bytes())?;
                self.stream
                    .shutdown(Shutdown::Both)
                    .expect("Shutdown failed");
                return Ok(());
            }
        };

        self.log(
            String::from("Handshake complete, websocket established."),
            log::LogLevel::Info,
        );

        // echo back whatever we get from here on
        loop {
            let recv: Vec<u8> = reader.fill_buf()?.to_vec();
            reader.consume(recv.len());
            let message = String::from_utf8(recv).unwrap();
            if !message.is_empty() {
                print!("{}", message);
                _ = self.stream.write_all(message.as_bytes());
            }
        }
    }
}

impl<S: Stream> ServerHandle<S> {
    /// Returns a result with either a valid value for Sec-WebSocket-Accept, or a string to be used
    /// in a 400 bad request
    fn validate_handshake(
        &self,
        client_handshake: String,
        hostname: String,
    ) -> Result<String, String> {
        self.log(
            format!("Validating client handshake {}", client_handshake),
            log::LogLevel::Debug,
        );
        let mut components = client_handshake.trim().split('\n');
        // pop the method + path + http version
        let http_request = match components.next() {
            Some(r) => r,
            None => return Err(String::from("Handshake is not a valid HTTP request")),
        };

        // validation 1 - must be a GET request, with a valid Request-URI with HTTP/1.1 or higher
        let mut request_components = http_request.split_whitespace();

        let mut err = String::from("Handshake is not a GET Request");
        match request_components.next() {
            Some("GET") => {}
            Some(_) => return Err(err),
            None => return Err(err),
        }

        // TODO: not validating the URI yet: https://www.rfc-editor.org/rfc/rfc6455#section-3
        err = String::from("Handshake contains invalid URI resource");
        if request_components.next().is_none() {
            return Err(err);
        }

        err =
            String::from("Handshake is using an invalid HTTP version, must be HTTP/1.1 or higher");
        match request_components.next() {
            Some(http) => {
                let c = http.split_once('/');
                match c {
                    Some(("HTTP", "1.1")) | Some(("HTTP", "2")) | Some(("HTTP", "3")) => {}
                    Some(_) => return Err(err),
                    None => return Err(err),
                };
            }
            None => return Err(err),
        };

        // TODO: I'm just chucking the rest of the headers here, but I could return them as part
        // of a tuple or struct or something, then pass back to a closure on the `handle_client`
        // and `listen` funcs.
        // e.g. the api is something like:
        // WebSocketClient::create("...").listen(on_initial_conn: { request }, on_recv: { bytes })
        // ergonomics wise I could also register those callbacks using their own funcs
        // or both, both is good
        let headers = components
            .filter_map(|header| header.split_once(':'))
            .map(|(header_name, val)| (header_name.trim().to_lowercase(), val.trim()))
            .collect::<HashMap<_, _>>();

        // validation 2 - must include a Host header matching server
        match headers.get("host") {
            Some(given_host) if *given_host.trim().to_string() == hostname => {}
            Some(_) => return Err(String::from("Invalid hostname")),
            None => return Err(String::from("Handshake missing Host header")),
        };

        // validation 3 - must include "upgrade: websocket" header
        match headers.get("upgrade") {
            Some(ug) if ug.to_lowercase() == "websocket" => {}
            Some(_) => return Err(String::from("Requested Upgrade was not 'websocket'")),
            None => return Err(String::from("Handshake missing Upgrade header")),
        };

        // validation 4 - must include "connection: upgrade" header
        match headers.get("connection") {
            Some(conn) if conn.to_lowercase() == "upgrade" => {}
            Some(_) => return Err(String::from("Requested Connection was not 'upgrade'")),
            None => return Err(String::from("Handshake missing Connection header")),
        };

        // validation 6 - "sec-websocket-version: 13". Process before key to avoid the hash if we can
        // NOTE: the RFC does allow for multiple version support: https://www.rfc-editor.org/rfc/rfc6455#section-4.4
        // but that is out of scope for this little toy (right now)
        match headers.get("sec-websocket-version") {
            Some(&"13") => {}
            Some(_) => return Err(String::from("Requested Sec-WebSocket-Version was not '13'")),
            None => {
                return Err(String::from(
                    "Handshake missing Sec-WebSocket-Version header",
                ))
            }
        };

        // validation 5 - key
        // This key must be exactly 24 characters (b64 on a 16 byte nonce), as per
        // https://www.rfc-editor.org/rfc/rfc6455#section-4.1
        let mut key = match headers.get("sec-websocket-key") {
            Some(h) => h.trim().to_string(),
            None => return Err(String::from("Handshake missing Sec-WebSocket-Key header")),
        };

        if key.chars().count() != 24 {
            return Err(String::from("Invalid Sec-WebSocket-Key"));
        }

        // the magic UUID from https://www.rfc-editor.org/rfc/rfc6455#section-1.3
        key += "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

        // TODO: I would like to write a pure-rust version of this myself, but right now I'm cheating and
        // just calling into rustcrypto
        let hash = Sha1::digest(key.as_bytes());
        let base64_hash = Base64::encode_string(&hash);
        Ok(base64_hash)
    }

    fn log(&self, msg: String, level: log::LogLevel) {
        // NOTE: this expect is half-reasonable since if we can't get a peer addr how are we
        // connected, but it should probably be handled more gracefully
        log::log(
            format!(
                "{} - {msg}",
                self.stream.peer_addr().expect("No peer address found")
            ),
            level,
        );
    }
}

#[cfg(test)]
mod tests {
    struct MockStream {}
    use std::net::{IpAddr, Ipv4Addr};

    use crate::client;
    use crate::server::*;
    impl Stream for MockStream {
        fn peer_addr(&self) -> std::io::Result<std::net::SocketAddr> {
            Ok(std::net::SocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                4024,
            ))
        }
    }
    fn make_test_handle() -> ServerHandle<MockStream> {
        ServerHandle {
            stream: MockStream {},
        }
    }

    #[test]
    fn valid_handshake() {
        let server = make_test_handle();

        assert_eq!(
            server.validate_handshake(
                client::HARDCODED_HANDSHAKE.to_string(),
                String::from("127.0.0.1:4024")
            ),
            Ok(String::from("s3pPLMBiTxaQ9kYGzzhZRbK+xOo="))
        );
    }

    #[test]
    fn malformed_request() {
        let server = make_test_handle();

        assert_eq!(
            server.validate_handshake(String::from("POST /ws HTTP/1.1"), String::from("localhost")),
            Err(String::from("Handshake is not a GET Request"))
        );
        assert_eq!(
            server.validate_handshake(String::from("GET /ws PTTH/1.1"), String::from("localhost")),
            Err(String::from(
                "Handshake is using an invalid HTTP version, must be HTTP/1.1 or higher"
            ))
        );
        assert_eq!(
            server.validate_handshake(String::from("GET /ws HTTP/1.0"), String::from("localhost")),
            Err(String::from(
                "Handshake is using an invalid HTTP version, must be HTTP/1.1 or higher"
            ))
        );
    }

    #[test]
    fn bad_host_header() {
        let server = make_test_handle();

        assert_eq!(
            server.validate_handshake(String::from("GET /ws HTTP/1.1"), String::from("localhost")),
            Err(String::from("Handshake missing Host header"))
        );
        assert_eq!(
            server.validate_handshake(
                String::from(
                    "GET /ws HTTP/1.1
            Host: badhost"
                ),
                String::from("localhost")
            ),
            Err(String::from("Invalid hostname"))
        );
    }

    #[test]
    fn bad_upgrade_header() {
        let server = make_test_handle();

        assert_eq!(
            server.validate_handshake(
                String::from(
                    "GET /ws HTTP/1.1
                    Host: localhost"
                ),
                String::from("localhost")
            ),
            Err(String::from("Handshake missing Upgrade header"))
        );
        assert_eq!(
            server.validate_handshake(
                String::from(
                    "GET /ws HTTP/1.1
                    Host: localhost
                    Upgrade: Not Websocket"
                ),
                String::from("localhost")
            ),
            Err(String::from("Requested Upgrade was not 'websocket'"))
        );
    }

    #[test]
    fn bad_connection_header() {
        let server = make_test_handle();

        assert_eq!(
            server.validate_handshake(
                String::from(
                    "GET /ws HTTP/1.1
            Host: localhost
            Upgrade: Websocket"
                ),
                String::from("localhost")
            ),
            Err(String::from("Handshake missing Connection header"))
        );
        assert_eq!(
            server.validate_handshake(
                String::from(
                    "GET /ws HTTP/1.1
            Host: localhost
            Upgrade: Websocket
            Connection: Not Upgrade"
                ),
                String::from("localhost")
            ),
            Err(String::from("Requested Connection was not 'upgrade'"))
        );
    }

    #[test]
    fn bad_version_header() {
        let server = make_test_handle();

        assert_eq!(
            server.validate_handshake(
                String::from(
                    "GET /ws HTTP/1.1
                    Host: localhost
                    Upgrade: Websocket
                    Connection: Upgrade"
                ),
                String::from("localhost")
            ),
            Err(String::from(
                "Handshake missing Sec-WebSocket-Version header"
            ))
        );
        assert_eq!(
            server.validate_handshake(
                String::from(
                    "GET /ws HTTP/1.1
                    Host: localhost
                    Upgrade: Websocket
                    Connection: Upgrade
                    Sec-WebSocket-Version: 14"
                ),
                String::from("localhost")
            ),
            Err(String::from("Requested Sec-WebSocket-Version was not '13'"))
        );
    }

    #[test]
    fn bad_key_header() {
        let server = make_test_handle();

        assert_eq!(
            server.validate_handshake(
                String::from(
                    "GET /ws HTTP/1.1
                    Host: localhost
                    Upgrade: Websocket
                    Connection: Upgrade
                    Sec-WebSocket-Version: 13"
                ),
                String::from("localhost")
            ),
            Err(String::from("Handshake missing Sec-WebSocket-Key header"))
        );
        assert_eq!(
            server.validate_handshake(
                String::from(
                    "GET /ws HTTP/1.1
                    Host: localhost
                    Upgrade: Websocket
                    Connection: Upgrade
                    Sec-WebSocket-Version: 13
                    Sec-WebSocket-Key: foo"
                ),
                String::from("localhost")
            ),
            Err(String::from("Invalid Sec-WebSocket-Key"))
        );
    }
}
