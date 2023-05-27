use chrono::Local;
use std::net::UdpSocket;

pub fn current_time() -> u32 {
    let dt = Local::now();
    dt.timestamp() as u32
}

pub fn get_local_ip() -> Option<String> {
    let skt = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(_) => return None,
    };

    match skt.connect("8.8.8.8:80") {
        Ok(()) => (),
        Err(_) => return None,
    };

    match skt.local_addr() {
        Ok(addr) => Some(addr.ip().to_string()),
        Err(_) => None,
    }
}
