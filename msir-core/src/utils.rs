use chrono::Local;

pub fn current_time() -> u32 {
    let dt = Local::now();
    dt.timestamp() as u32
}
