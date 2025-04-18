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
        return create_server();
    } else if run_mode.to_lowercase() == "client" {
        let bind_addr: &str;
        if args.iter().count() < 3 {
            bind_addr = "127.0.0.1:4024";
        } else {
            bind_addr = &args[2];
        }
        return create_client(bind_addr);
    } else {
        panic!("Must give arg as 'client' or 'server'")
    }
}

fn create_client(addr: &str) -> std::io::Result<()> {
    let mut stream = TcpStream::connect(addr)?;
    stream.write(HARDCODED_HANDSHAKE)?;
    Ok(())
}

fn create_server() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:4024")?;

    for stream in listener.incoming() {
        handle_client(stream?);
    }
    Ok(())
}

fn handle_client(mut stream: TcpStream) {
    let mut strbuf = String::new();
    stream.read_to_string(&mut strbuf).unwrap();
    println!("{}", strbuf);
}
