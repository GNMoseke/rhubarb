use base64ct::{Base64, Encoding};
use sha1::{Digest, Sha1};
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    net::{Shutdown, TcpListener, TcpStream},
};

pub(crate) struct WebSocketServer<L: Listener> {
    _listener: L,
    hostname: String,
}

pub(crate) trait Listener {}
impl Listener for TcpListener {}

impl WebSocketServer<TcpListener> {
    pub(crate) fn create(bind_addr: &str) -> std::io::Result<WebSocketServer<TcpListener>> {
        let _listener = TcpListener::bind(bind_addr)?;
        Ok(WebSocketServer {
            _listener,
            hostname: bind_addr.to_string(),
        })
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

impl<L: Listener> WebSocketServer<L> {
    /// Returns a result with either a valid value for Sec-WebSocket-Accept, or a string to be used
    /// in a 400 bad request
    fn validate_handshake(&self, client_handshake: String) -> Result<String, String> {
        let mut components = client_handshake.trim().split('\n');
        // pop the method + path + http version
        let http_request = match components.next() {
            Some(r) => r,
            None => return Err(String::from("Handshake is not a valid HTTP request")),
        };

        // validation 1 - must be a GET request, with a valid Request-URI with HTTP/1.1 or higher
        let mut request_components = http_request.trim().split_whitespace().into_iter();

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
            Some(given_host) if given_host.trim().to_string() == self.hostname => {}
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
        return Ok(base64_hash);
    }
}

#[cfg(test)]
mod tests {
    struct MockListener {}
    use crate::client;
    use crate::server::*;
    impl Listener for MockListener {}
    fn make_test_server() -> WebSocketServer<MockListener> {
        WebSocketServer {
            _listener: MockListener {},
            hostname: String::from("localhost"),
        }
    }

    #[test]
    fn valid_handshake() {
        let server = make_test_server();

        assert_eq!(
            server.validate_handshake(client::HARDCODED_HANDSHAKE.to_string()),
            Ok(String::from("s3pPLMBiTxaQ9kYGzzhZRbK+xOo="))
        );
    }

    #[test]
    fn malformed_request() {
        let server = make_test_server();

        assert_eq!(
            server.validate_handshake(String::from("POST /ws HTTP/1.1")),
            Err(String::from("Handshake is not a GET Request"))
        );
        assert_eq!(
            server.validate_handshake(String::from("GET /ws PTTH/1.1")),
            Err(String::from(
                "Handshake is using an invalid HTTP version, must be HTTP/1.1 or higher"
            ))
        );
        assert_eq!(
            server.validate_handshake(String::from("GET /ws HTTP/1.0")),
            Err(String::from(
                "Handshake is using an invalid HTTP version, must be HTTP/1.1 or higher"
            ))
        );
    }

    #[test]
    fn bad_host_header() {
        let server = make_test_server();

        assert_eq!(
            server.validate_handshake(String::from("GET /ws HTTP/1.1")),
            Err(String::from("Handshake missing Host header"))
        );
        assert_eq!(
            server.validate_handshake(String::from(
                "GET /ws HTTP/1.1
            Host: badhost"
            )),
            Err(String::from("Invalid hostname"))
        );
    }

    #[test]
    fn bad_upgrade_header() {
        let server = make_test_server();

        assert_eq!(
            server.validate_handshake(String::from(
                "GET /ws HTTP/1.1
            Host: localhost"
            )),
            Err(String::from("Handshake missing Upgrade header"))
        );
        assert_eq!(
            server.validate_handshake(String::from(
                "GET /ws HTTP/1.1
            Host: localhost
            Upgrade: Not Websocket"
            )),
            Err(String::from("Requested Upgrade was not 'websocket'"))
        );
    }

    #[test]
    fn bad_connection_header() {
        let server = make_test_server();

        assert_eq!(
            server.validate_handshake(String::from(
                "GET /ws HTTP/1.1
            Host: localhost
            Upgrade: Websocket"
            )),
            Err(String::from("Handshake missing Connection header"))
        );
        assert_eq!(
            server.validate_handshake(String::from(
                "GET /ws HTTP/1.1
            Host: localhost
            Upgrade: Websocket
            Connection: Not Upgrade"
            )),
            Err(String::from("Requested Connection was not 'upgrade'"))
        );
    }

    #[test]
    fn bad_version_header() {
        let server = make_test_server();

        assert_eq!(
            server.validate_handshake(String::from(
                "GET /ws HTTP/1.1
            Host: localhost
            Upgrade: Websocket
            Connection: Upgrade"
            )),
            Err(String::from(
                "Handshake missing Sec-WebSocket-Version header"
            ))
        );
        assert_eq!(
            server.validate_handshake(String::from(
                "GET /ws HTTP/1.1
            Host: localhost
            Upgrade: Websocket
            Connection: Upgrade
            Sec-WebSocket-Version: 14"
            )),
            Err(String::from("Requested Sec-WebSocket-Version was not '13'"))
        );
    }

    #[test]
    fn bad_key_header() {
        let server = make_test_server();

        assert_eq!(
            server.validate_handshake(String::from(
                "GET /ws HTTP/1.1
            Host: localhost
            Upgrade: Websocket
            Connection: Upgrade
            Sec-WebSocket-Version: 13"
            )),
            Err(String::from("Handshake missing Sec-WebSocket-Key header"))
        );
        assert_eq!(
            server.validate_handshake(String::from(
                "GET /ws HTTP/1.1
            Host: localhost
            Upgrade: Websocket
            Connection: Upgrade
            Sec-WebSocket-Version: 13
            Sec-WebSocket-Key: foo"
            )),
            Err(String::from("Invalid Sec-WebSocket-Key"))
        );
    }
}
