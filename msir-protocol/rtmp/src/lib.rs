pub fn add(left: usize, right: usize) -> usize {
    left + right
}

pub mod chunk;
pub mod connection;
pub mod error;
pub mod handshake;
pub mod message;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
        let s2 = message::types::user_ctrl_ev_type::STREAM_BEGIN;
    }

    #[test]
    fn hs_works() {
        let chs = handshake::hs_server::ComplexHandshakeServer {};
        let shs = handshake::hs_server::SimpleHandshakeServer {};
        println!("Lukie {} {}", chs.print(), shs.print());
    }
}
