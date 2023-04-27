use byteorder::{BigEndian, ByteOrder};
use bytes::{Bytes, BytesMut};
use rand::Rng;
use tokio::{net::TcpStream, io::AsyncReadExt};
use super::{error::HandshakeError, RTMP_HANDSHAKE_SIZE, RTMP_VERSION};

pub struct Context {
    // [1+1536]
    pub c0c1: BytesMut,
    // [1+1536+1536]
    pub s0s1s2: BytesMut,
    // [1536]
    pub c2: BytesMut,
}

impl Context {
    pub fn new() -> Self {
        Self {
            c0c1: BytesMut::new(),
            s0s1s2: BytesMut::new(),
            c2: BytesMut::new(),
        }
    }
    pub async fn read_c0c1(&mut self, io: &mut TcpStream) -> Result<(), HandshakeError> {
        if self.c0c1.is_empty() {
            self.c0c1.resize(RTMP_HANDSHAKE_SIZE+1, 0);
            io.read_exact(&mut self.c0c1).await?;
        }
        Ok(())
    }
    pub async fn read_s0s1s2(&mut self, io: &mut TcpStream) -> Result<(), HandshakeError> {
        if self.s0s1s2.is_empty() {
            self.s0s1s2.resize(1+RTMP_HANDSHAKE_SIZE*2, 0);
            io.read_exact(&mut self.s0s1s2).await?;
        }
        Ok(())
    }
    pub async fn read_c2(&mut self, io: &mut TcpStream) -> Result<(), HandshakeError> {
        if self.c2.is_empty() {
            self.c2.resize(RTMP_HANDSHAKE_SIZE, 0);
            io.read_exact(&mut self.c2).await?;
        }
        Ok(())
    }
    pub fn create_c0c1(&mut self) -> Result<(), HandshakeError> {
        if self.c0c1.is_empty() {
            self.c0c1.reserve(1+RTMP_HANDSHAKE_SIZE);
            self.c0c1.extend_from_slice(&[RTMP_VERSION]);
            let mut ts = [0_u8; 4];
            BigEndian::write_u32(&mut ts, current_time());
            self.c0c1.extend_from_slice(&ts);
            self.c0c1.extend_from_slice(&[0; 4]);
            let mut rng = rand::thread_rng();
            for _ in 0..(RTMP_HANDSHAKE_SIZE-8) {
                self.c0c1.extend_from_slice(&[rng.gen()]);
            }
        }
        Ok(())
    }
    pub fn create_s0s1s2(&mut self) -> Result<(), HandshakeError> {
        if self.s0s1s2.is_empty() {
            self.s0s1s2.reserve(1+RTMP_HANDSHAKE_SIZE*2);
            self.s0s1s2.extend_from_slice(&[RTMP_VERSION]);
            let mut ts = [0_u8; 4];
            BigEndian::write_u32(&mut ts, current_time());
            self.s0s1s2.extend_from_slice(&ts);
            if self.c0c1.is_empty() {
                self.s0s1s2.extend_from_slice(&[0; 4]);
            } else {
                self.s0s1s2.extend_from_slice(&self.c0c1[1..5]);
            }
            let mut rng = rand::thread_rng();
            for _ in 0..(RTMP_HANDSHAKE_SIZE-8) {
                self.s0s1s2.extend_from_slice(&[rng.gen()]);
            }
            if self.c0c1.is_empty() {
                for _ in 0..(RTMP_HANDSHAKE_SIZE) {
                    self.s0s1s2.extend_from_slice(&[rng.gen()]);
                }
            } else {
                self.s0s1s2.extend_from_slice(&self.c0c1[1..RTMP_HANDSHAKE_SIZE+1]);
            }
        }
        Ok(())
    }
    pub fn create_c2(&mut self) -> Result<(), HandshakeError> {
        if self.c2.is_empty() {
            self.c2.reserve(RTMP_HANDSHAKE_SIZE);
            let mut ts = [0_u8; 4];
            BigEndian::write_u32(&mut ts, current_time());
            self.c2.extend_from_slice(&ts);
            if self.s0s1s2.is_empty() {
                self.c2.extend_from_slice(&[0; 4]);
            } else {
                self.c2.extend_from_slice(&self.s0s1s2[1..5]);
            }
            let mut rng = rand::thread_rng();
            for _ in 0..(RTMP_HANDSHAKE_SIZE-8) {
                self.c0c1.extend_from_slice(&[rng.gen()]);
            }
        }
        Ok(())
    }
}

use chrono::Local;
fn current_time() -> u32 {
    let dt = Local::now();
    dt.timestamp() as u32
}