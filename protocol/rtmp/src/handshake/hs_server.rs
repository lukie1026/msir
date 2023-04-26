use super::hs_trait::THandshakeServer;

pub struct SimpleHandshakeServer {}

pub struct ComplexHandshakeServer {}

impl SimpleHandshakeServer {
    // pub fn new() -> Self {
    //     Self {}
    // }
    pub fn print(&self) -> usize {
        self.aaa()
    }
}

impl ComplexHandshakeServer {
    // pub fn new() -> Self {
    //     Self {}
    // }
    pub fn print(&self) -> usize {
        self.aaa()
    }
}

impl THandshakeServer for SimpleHandshakeServer {
    // fn aaa(& self) -> usize {
    //     111
    // }
    fn bbb(&self) -> usize {
        222
    }
    fn ccc(&self) -> usize {
        333
    }
}

impl THandshakeServer for ComplexHandshakeServer {
    // fn aaa(& self) -> usize {
    //     444
    // }
    fn bbb(&self) -> usize {
        555
    }
    fn ccc(&self) -> usize {
        666
    }
}
