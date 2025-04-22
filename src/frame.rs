use std::{collections::VecDeque, io::Read};

struct WebSocketFrame {
    fin: bool,
    masked: bool,
    opcode: WebSocketOpCode,
    payload_len: u64,
    mask_key: Option<[u8; 4]>,
    data: Vec<u8>,
}

enum WebSocketOpCode {
    Continuation,
    Text,
    Binary,
    Close,
    Ping,
    Pong,
    Reserved,
}

impl WebSocketFrame {
    fn new_bin(fin: bool, opcode: WebSocketOpCode, data: Vec<u8>, mask_key: Option<[u8; 4]>) -> WebSocketFrame {
        let len: u64 = (data.len() * 8).try_into().unwrap();
        if mask_key != None {
            return WebSocketFrame { fin: fin, masked: true, opcode: opcode, payload_len: len, mask_key: mask_key, data: data}
        }
        WebSocketFrame { fin: fin, masked: false, opcode: opcode, payload_len: len, mask_key: mask_key, data: data }
    }

    fn new_str(fin: bool, opcode: WebSocketOpCode, data: String, mask_key: Option<[u8; 4]>) -> WebSocketFrame {
        let len: u64 = (data.len() * 8).try_into().unwrap();
        if mask_key != None {
            return WebSocketFrame { fin: fin, masked: true, opcode: opcode, payload_len: len, mask_key: mask_key, data: data.as_bytes().into() }
        }
        WebSocketFrame { fin: fin, masked: false, opcode: opcode, payload_len: len, mask_key: mask_key, data: data.as_bytes().into() }
    }
    /// Parse a frame from raw bytes incoming on the wire
    /// https://www.rfc-editor.org/rfc/rfc6455#section-5.2
    fn parse(raw: Vec<u8>) -> WebSocketFrame {
        // FIXME: get rid of panics and expects and gracefully handle malformed frames

        let mut handle = VecDeque::from(raw);

        // first byte is metadata: fin bit, 2 reserved, opcode
        let meta = handle.pop_front().expect("frame contained fin and opcode");
        let fin = match meta >> 7 {
            0 => false,
            1 => true,
            _ => panic!("failed bitshift"),
        };

        let opcode = match (meta & !0xF0) | (0x0 & 0xF0) {
            0x0 => WebSocketOpCode::Continuation,
            0x1 => WebSocketOpCode::Text,
            0x2 => WebSocketOpCode::Binary,
            0x3..=0x7 => WebSocketOpCode::Reserved,
            0x8 => WebSocketOpCode::Close,
            0x9 => WebSocketOpCode::Ping,
            0xA => WebSocketOpCode::Pong,
            0xB..=0xF => WebSocketOpCode::Reserved,
            _ => panic!("failed mask"),
        };

        let mask_and_len = handle
            .pop_front()
            .expect("frame contained mask flag and initial length");
        let masked = match mask_and_len >> 7 {
            0 => false,
            1 => true,
            _ => panic!("failed bitshift"),
        };

        let shifted_len = (mask_and_len & !0x80) | (0x0 & 0x80);
        let payload_len: u64 = match shifted_len {
            0..=125 => shifted_len.into(),
            126 => {
                let mut len_bytes: u64 = handle.pop_front().expect("length bytes").into();
                len_bytes += u64::from(handle.pop_front().expect("length bytes"));
                len_bytes
            }
            127 => {
                // TODO: there's a clever way to do this with an iterator and a take(8) but I can't
                // find it right now
                let mut len_bytes: u64 = 0;
                (0..8).for_each(|_| {
                    len_bytes += u64::from(handle.pop_front().expect("length bytes"));
                });
                len_bytes
            }
            _ => panic!(),
        };

        let mask_key = if masked {
            let mask: [u8; 4] = handle
                .drain(0..4)
                .collect::<Vec<u8>>()
                .try_into()
                .expect("4 byte mask key");
            Some(mask)
        } else {
            None
        };

        let mut data_buf = vec![0u8; payload_len as usize];
        handle.read_exact(&mut data_buf).expect("read length bytes");

        WebSocketFrame {
            fin,
            masked,
            opcode,
            payload_len,
            mask_key,
            data: data_buf.into(),
        }
    }

    fn encode(self) -> Vec<u8> {
        let mut bytes: Vec<u8> = Vec::new();

        // first byte is fin + empty + opcode most significant -> least significant
        let mut meta: u8 = 0;
        
        if self.fin {
            meta = meta | 0x80;
        }

        bytes.push(meta);

        match self.opcode {
            WebSocketOpCode::Continuation => meta = meta | 0x01,
            WebSocketOpCode::Text => meta = meta | 0x02,
            WebSocketOpCode::Binary => meta = meta | 0x03,
            WebSocketOpCode::Close => meta = meta | 0x08,
            WebSocketOpCode::Ping => meta = meta | 0x09,
            WebSocketOpCode::Pong => meta = meta | 0x0A,
            WebSocketOpCode::Reserved => meta = meta | 0x0F,
        };

        if self.payload_len == 126 {
            // push 2 bytes of length after the magic 126
            bytes.push(126);

        }
        else if self.payload_len >= 127 {
            // push 8 bytes of length after the magic 127

        }
        else { bytes.push(self.payload_len.try_into().expect("len is 8 bits")) }

        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_and_parse() {
        let frame = WebSocketFrame::new_bin(true, WebSocketOpCode::Continuation, vec![0], None);
        frame.encode();
    }
}
