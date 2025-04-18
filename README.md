# Overview
An implementation of the [WebSocket Protocol](https://www.rfc-editor.org/rfc/rfc6455)
in pure rust (with [one exception](#websocket-key)), because I've realized that my rust
ability is mostly taping existing libraries together instead of knowing how to
actually leverage the language.

## WebSocket Key
For the `Sec-WebSocket-Key` handshake, I'm using the [sha1](https://docs.rs/sha1/latest/sha1/)
and [base64ct](https://docs.rs/base64ct/latest/base64ct/) crates, because my
main focus with this project is on the websocket protocol. I may come back and
write my own implementation in the future, but it's not the goal of this toy.
