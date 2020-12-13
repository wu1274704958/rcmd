use std::sync::Arc;
use std::sync::Mutex;
use std::collections::HashMap;
use crate::ab_client::AbClient;
use std::collections::hash_map::RandomState;
use crate::agreement::Message;

pub trait Handle<'a> {
    type ABClient;
    type Id;
    type Data;
    fn handle(&self,data:&'a Self::Data,clients:&'a mut Arc<Mutex<HashMap<usize,Box<Self::ABClient>>>>,id:Self::Id)-> Vec<u8> where Self::Id:Copy;
}

#[derive(Copy, Clone)]
pub struct TestHandler{

}

impl<'a> Handle<'a> for TestHandler {
    type ABClient = AbClient;
    type Id = usize;
    type Data = Message<'a>;

    fn handle(&self, data: &'a Message<'a>, clients: &'a mut Arc<Mutex<HashMap<usize, Box<Self::ABClient>>>>,id:Self::Id) -> Vec<u8> {
        return vec![b'{',b'"',b'r',b'e',b't',b'"',b':',b'0',b'}'];
    }
}

