use std::env;
use client::*;
use server::*;

mod server;
mod client;

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
        _ = client.send(HARDCODED_HANDSHAKE.as_bytes());

        // now read user stdin and send that
        let mut stdin_buf = String::new();
        let stdin = std::io::stdin();
        while stdin.read_line(&mut stdin_buf)? != 0 {
            _ = client.send(stdin_buf.as_bytes());
            stdin_buf.clear();
        }
        Ok(())
    } else {
        panic!("Must give arg as 'client' or 'server'")
    }
}
