use tokio::net::TcpStream;
use std::collections::HashMap;
use tokio::time::Duration;

struct ReaderWriter {
    io: TcpStream,
    snd_tm: Duration,
    rcv_tm: Duration,
}

impl ReaderWriter {
    fn read(&self) {

    }
}

pub struct Stack {
    // For peer in/out
    io: ReaderWriter,
    requests: HashMap<f64, String>,
    // For peer in
    // For peer out
}