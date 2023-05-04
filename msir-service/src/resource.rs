use std::{
    collections::HashMap,
    sync::mpsc::{Receiver, Sender},
};

use rtmp::message::RtmpMessage;
use tokio::sync;

use crate::error::ServiceError;

pub enum RoleType {
    Consumer,
    Producer,
}

pub enum Token {
    ProducerToken(Sender<RtmpMessage>),
    ComsumerToken(Receiver<RtmpMessage>),
}

pub struct Manager {
    pool: HashMap<String, Resource>,
}

impl Manager {
    // pub fn register(&mut self, id: String, role: RoleType) -> Result<Token, ServiceError> {

    // }
}

pub struct Resource {}
