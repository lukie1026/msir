use bytes::Bytes;

pub fn is_video_sequence_header(data: &Bytes) -> bool {
    // This is assuming h264.
    return data.len() >= 2 && data[0] == 0x17 && data[1] == 0x00;
}

pub fn is_audio_sequence_header(data: &Bytes) -> bool {
    // This is assuming aac
    return data.len() >= 2 && data[0] == 0xaf && data[1] == 0x00;
}

pub fn is_video_keyframe(data: &Bytes) -> bool {
    // assumings h264
    return data.len() >= 2 && data[0] == 0x17 && data[1] != 0x00; // 0x00 is the sequence header, don't count that for now
}
