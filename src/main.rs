use std::{
    env,
    io::{BufRead, BufReader, Write},
    net::{Shutdown, TcpListener, TcpStream},
};

const HARDCODED_HANDSHAKE: &[u8] = b"
GET /ws HTTP/1.1
Host: localhost
Upgrade: websocket
Connection: Upgrade
Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==
Origin: localhost
Sec-WebSocket-Protocol: rhubarb
Sec-WebSocket-Version: 13
";

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.iter().count() < 2 {
        panic!("Must give arg as 'client' or 'server'")
    }
    let run_mode = &args[1];

    if run_mode.to_lowercase() == "server" {
        let server = WebSocketServer::create("127.0.0.1:4024")?;
        return server.listen();
    } else if run_mode.to_lowercase() == "client" {
        let bind_addr: &str;
        if args.iter().count() < 3 {
            bind_addr = "127.0.0.1:4024";
        } else {
            bind_addr = &args[2];
        }
        let mut client = WebSocketClient::create(bind_addr)?;
        client.send(HARDCODED_HANDSHAKE);
        
        // now read user stdin and send that
        let mut stdin_buf = String::new();
        let stdin = std::io::stdin();
        while stdin.read_line(&mut stdin_buf)? != 0 {
            client.send(stdin_buf.as_bytes());
            stdin_buf.clear();
        }
        Ok(())
    } else {
        panic!("Must give arg as 'client' or 'server'")
    }
}

struct WebSocketServer {
    _listener: TcpListener,
}

struct WebSocketClient {
    _stream: TcpStream,
}

impl WebSocketClient {
    fn create(bind_addr: &str) -> std::io::Result<WebSocketClient> {
        let _stream = TcpStream::connect(bind_addr)?;
        Ok(WebSocketClient { _stream })
    }

    fn send(&mut self, data: &[u8]) -> std::io::Result<()> {
        self._stream.write(data)?;
        Ok(())
    }
}

impl WebSocketServer {
    fn create(bind_addr: &str) -> std::io::Result<WebSocketServer> {
        let _listener = TcpListener::bind(bind_addr)?;
        Ok(WebSocketServer { _listener })
    }

    fn listen(self) -> std::io::Result<()> {
        for stream in self._listener.incoming() {
            if let Ok(stream) = stream {
                // TODO: dispatch each client to its own thread so that one bad handshake doesn't take
                // down the server
                self.handle_client(stream)?;
            }
        }
        Ok(())
    }

    fn handle_client(&self, mut stream: TcpStream) -> std::io::Result<()> {
        let mut reader = BufReader::new(stream.try_clone()?);
        let recv: Vec<u8> = reader.fill_buf()?.to_vec();
        reader.consume(recv.len());

        // need to first handle the handshake, then start processing data
        let handshake = String::from_utf8(recv).map_err(|_| {
            // TODO: handle a failed shutdown
            stream.shutdown(Shutdown::Both);
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
                stream.write(message.as_bytes());
            }
        }
    }
}
