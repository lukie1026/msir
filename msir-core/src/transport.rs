use std::{io, time::Duration};
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufStream},
    net::TcpStream,
    time::{error::Elapsed, timeout},
};
use tracing::{debug, error, info, instrument, trace, warn};

pub static NOTIMEOUT: Duration = Duration::MAX;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("Read closed by peer")]
    EndOfFile,

    #[error("An IO error occurred: {0}")]
    Io(#[from] io::Error),

    #[error("Timeout: {0}")]
    Timeout(#[from] Elapsed),
}

type Result<T> = std::result::Result<T, TransportError>;

pub struct Transport {
    io: BufStream<TcpStream>,
    recv_timeout: Duration,
    send_timeout: Duration,
    recv_bytes: u64,
    send_bytes: u64,

    buf: Vec<u8>,
    safe: bool,
    write_pos: usize,
    real_read_pos: usize,
    virtual_read_pos: usize,
}

impl Transport {
    pub fn new(io: TcpStream) -> Self {
        let mut buf = Vec::with_capacity(131072);
        unsafe {
            buf.set_len(131072);
        }
        Self {
            io: BufStream::with_capacity(0, 131072, io),
            recv_timeout: NOTIMEOUT,
            send_timeout: NOTIMEOUT,
            recv_bytes: 0,
            send_bytes: 0,

            buf,
            safe: false,
            write_pos: 0,
            real_read_pos: 0,
            virtual_read_pos: 0,
        }
    }

    pub fn safe_guard(&mut self) {
        if self.real_read_pos - self.virtual_read_pos > 0 {
            trace!("Safe guard restore");
        }
        self.real_read_pos = self.virtual_read_pos;
        self.safe = true;
    }

    pub fn safe_flush(&mut self) {
        self.safe = false;
        self.virtual_read_pos = self.real_read_pos;
    }

    pub fn set_recv_timeout(&mut self, tm: Duration) {
        self.recv_timeout = tm;
    }

    pub fn set_send_timeout(&mut self, tm: Duration) {
        self.send_timeout = tm;
    }

    pub fn get_recv_bytes(&mut self) -> u64 {
        self.recv_bytes
    }

    pub fn get_send_bytes(&mut self) -> u64 {
        self.send_bytes
    }

    fn buf_len(&mut self) -> usize {
        self.write_pos - self.real_read_pos
    }

    fn buf_left(&mut self) -> usize {
        self.buf.capacity() - self.write_pos
    }

    fn buf_move_to_head(&mut self) {
        trace!(
            "Readbuf moved, len={}, move={}",
            self.write_pos - self.virtual_read_pos,
            self.virtual_read_pos
        );
        let mut new_buf = Vec::with_capacity(131072);
        new_buf.extend_from_slice(&self.buf[self.virtual_read_pos..self.write_pos]);
        unsafe { new_buf.set_len(131072) }
        self.buf = new_buf;
        let offset = self.virtual_read_pos;
        self.virtual_read_pos = 0;
        self.real_read_pos -= offset;
        self.write_pos -= offset;
    }

    fn buf_reset(&mut self) {
        self.virtual_read_pos = 0;
        self.real_read_pos = 0;
        self.write_pos = 0;
    }

    pub async fn read_exact(&mut self, size: usize) -> Result<&[u8]> {
        if self.write_pos == self.virtual_read_pos {
            self.buf_reset();
        }
        while self.buf_len() < size {
            if self.buf_left() + self.buf_len() < size {
                self.buf_move_to_head();
            }
            if self.recv_timeout == NOTIMEOUT {
                match self.io.read(&mut self.buf[self.write_pos..]).await? {
                    0 => return Err(TransportError::EndOfFile),
                    n => self.write_pos += n,
                }
            } else {
                match timeout(
                    self.send_timeout,
                    self.io.read(&mut self.buf[self.write_pos..]),
                )
                .await??
                {
                    0 => return Err(TransportError::EndOfFile),
                    n => self.write_pos += n,
                }
            }
        }
        self.recv_bytes += size as u64;
        self.real_read_pos += size;
        if !self.safe {
            self.virtual_read_pos = self.real_read_pos;
        }
        Ok(&self.buf[self.real_read_pos - size..self.real_read_pos])
    }

    pub async fn read_u8(&mut self) -> Result<u8> {
        Ok(self.read_exact(1).await?[0])
    }

    pub async fn read_u32(&mut self) -> Result<u32> {
        let v = self.read_exact(4).await?;
        Ok((v[0] as u32) << 24 | (v[1] as u32) << 16 | (v[2] as u32) << 8 | (v[3] as u32))
    }

    pub async fn write_all(&mut self, buf: &[u8]) -> Result<()> {
        if self.send_timeout == NOTIMEOUT {
            Ok(self.io.write_all(buf).await.and_then(|ret| {
                self.send_bytes += buf.len() as u64;
                Ok(ret)
            })?)
        } else {
            Ok(timeout(self.send_timeout, self.io.write_all(buf))
                .await?
                .and_then(|ret| {
                    self.send_bytes += buf.len() as u64;
                    Ok(ret)
                })?)
        }
    }

    pub async fn flush(&mut self) -> Result<()> {
        Ok(self.io.flush().await?)
    }
}
