pub trait THandshakeServer {
    fn aaa(&self) -> usize {
        999
    }
    fn bbb(&self) -> usize;
    fn ccc(&self) -> usize;
}
