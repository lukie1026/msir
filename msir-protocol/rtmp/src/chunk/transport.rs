use bytes::{Buf, Bytes, BytesMut};
use std::{
    io::{self, Cursor},
    time::Duration,
};
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufStream},
    net::TcpStream,
    time::{error::Elapsed, timeout},
};
use tracing::{error, info, instrument, trace, warn};

pub static NOTIMEOUT: Duration = Duration::MAX;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("Read buffer has no space")]
    BufNoSpace,

    #[error("Read unexpected EOF")]
    ReadUnexpectedEof,

    #[error("End of file")]
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
        let mut new_buf = Vec::with_capacity(131072);
        new_buf.extend_from_slice(&self.buf[self.virtual_read_pos..self.write_pos]);
        unsafe { new_buf.set_len(131072) }
        self.buf = new_buf;
        let offset = self.virtual_read_pos;
        self.virtual_read_pos = 0;
        self.real_read_pos -= offset;
        self.write_pos -= offset;
    }

    pub async fn read_exact(&mut self, size: usize) -> Result<&[u8]> {
        while self.buf_len() < size {
            if self.buf_left() + self.buf_len() < size {
                self.buf_move_to_head();
            }
            match self.io.read(&mut self.buf[self.write_pos..]).await? {
                0 => return Err(TransportError::EndOfFile),
                n => self.write_pos += n,
            }
        }
        self.send_bytes += size as u64;
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

    // pub async fn read_u8(&mut self) -> Result<u8> {
    //     if self.recv_timeout == NOTIMEOUT {
    //         Ok(self.io.read_u8().await.and_then(|ret| {
    //             self.recv_bytes += 1;
    //             Ok(ret)
    //         })?)
    //     } else {
    //         Ok(timeout(self.recv_timeout, self.io.read_u8())
    //             .await?
    //             .and_then(|ret| {
    //                 self.recv_bytes += 1;
    //                 Ok(ret)
    //             })?)
    //     }
    // }

    // pub async fn read_u32(&mut self) -> Result<u32> {
    //     if self.recv_timeout == NOTIMEOUT {
    //         Ok(self.io.read_u32().await.and_then(|ret| {
    //             self.recv_bytes += 4;
    //             Ok(ret)
    //         })?)
    //     } else {
    //         Ok(timeout(self.recv_timeout, self.io.read_u32())
    //             .await?
    //             .and_then(|ret| {
    //                 self.recv_bytes += 4;
    //                 Ok(ret)
    //             })?)
    //     }
    // }

    // pub async fn read_exact(&mut self, buf: &mut [u8]) -> Result<usize> {
    //     if self.recv_timeout == NOTIMEOUT {
    //         Ok(self.io.read_exact(buf).await.and_then(|ret| {
    //             self.recv_bytes += ret as u64;
    //             Ok(ret)
    //         })?)
    //     } else {
    //         Ok(timeout(self.recv_timeout, self.io.read_exact(buf))
    //             .await?
    //             .and_then(|ret| {
    //                 self.recv_bytes += ret as u64;
    //                 Ok(ret)
    //             })?)
    //     }
    // }

    // pub async fn read_exact(&mut self, buf: &mut [u8]) -> Result<usize> {
    //     let mut nread = 0;
    //     while nread < buf.len() {
    //         nread += self.io.read(&mut buf[nread..]).await?;
    //         self.recv_bytes += nread as u64;
    //     }
    //     Ok(nread)
    // }

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

    // pub async fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
    //     if buf.len() == 0 {
    //         return Err(TransportError::BufNoSpace);
    //     }
    //     let nread;
    //     if self.recv_timeout == NOTIMEOUT {
    //         nread = self.io.read(buf).await?;
    //     } else {
    //         nread = timeout(self.recv_timeout, self.io.read(buf)).await??
    //     }

    //     if nread == 0 {
    //         return Err(TransportError::ReadUnexpectedEof)
    //     }
    //     self.recv_bytes += nread as u64;
    //     Ok(nread)
    // }

    // pub async fn write_all(&mut self, data: &[u8]) -> Result<()> {
    //     let mut buf = Cursor::new(data);
    //     if self.send_timeout == NOTIMEOUT {
    //         Ok(self.io.write_all_buf(&mut buf).await.and_then(|ret| {
    //             self.send_bytes += data.len() as u64;
    //             Ok(ret)
    //         })?)
    //     } else {
    //         Ok(timeout(self.send_timeout, self.io.write_all_buf(&mut buf))
    //             .await?
    //             .and_then(|ret| {
    //                 self.send_bytes += data.len() as u64;
    //                 Ok(ret)
    //             })?)
    //     }
    // }
}
