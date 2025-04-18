use std::{
    env,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
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
        let client = WebSocketClient::create(bind_addr)?;
        return client.send(HARDCODED_HANDSHAKE);
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

    fn send(mut self, data: &[u8]) -> std::io::Result<()> {
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
            self.handle_client(stream?);
        }
        Ok(())
    }

    fn handle_client(&self, mut stream: TcpStream) {
        // need to first handle the handshake, then start processing data
        let mut strbuf = String::new();
        stream.read_to_string(&mut strbuf).unwrap();
        println!("{}", strbuf);
    }
}
