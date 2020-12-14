use std::sync::Arc;
use std::sync::Mutex;
use std::collections::HashMap;
use crate::ab_client::AbClient;
use std::collections::hash_map::RandomState;
use crate::agreement::Message;
use std::future::Future;
use async_std::task::Context;
use tokio::macros::support::{Pin, Poll};


pub trait SubHandle :Send + Sync  {
    type ABClient;
    type Id;
    type Data;
    fn handle(&self,data:&Self::Data,clients:&Arc<Mutex<HashMap<Self::Id,Box<Self::ABClient>>>>,id:Self::Id)-> Option<Vec<u8>>
        where Self::Id :Copy;
}

pub trait Handle<T> where T:SubHandle
{
    fn handle_ex(&self,data:&<T as SubHandle>::Data,
                 clients:&Arc<Mutex<HashMap<<T as SubHandle>::Id,Box<<T as SubHandle>::ABClient>>>>,
                 id:<T as SubHandle>::Id)-> Option<Vec<u8>> where <T as SubHandle>::Id:Copy
    {
        for i in 0..self.handler_count()
        {
            let handler = self.get_handler(i);
            if let Some(v) = handler.handle(data,clients,id)
            {
                return Some(v);
            }
        }
        None
    }

    fn add_handler(&mut self,h:Arc<dyn SubHandle<ABClient = <T as SubHandle>::ABClient, Id = <T as SubHandle>::Id, Data = <T as SubHandle>::Data>>);
    fn handler_count(&self)->usize;
    fn get_handler(&self,i:usize)->&dyn SubHandle<ABClient = <T as SubHandle>::ABClient, Id = <T as SubHandle>::Id, Data = <T as SubHandle>::Data>;
}

#[derive(Clone)]
pub struct DefHandler<T> where T:SubHandle {
    handlers : Vec<Arc<dyn SubHandle<ABClient = <T as SubHandle>::ABClient, Id = <T as SubHandle>::Id, Data = <T as SubHandle>::Data>>>
}

impl<T> DefHandler<T>  where T:SubHandle {
    pub fn new()->DefHandler<T>
    {
        DefHandler::<T>{
            handlers:Vec::new()
        }
    }
}

impl<T> Handle<T> for DefHandler<T> where T:SubHandle{

    fn add_handler(&mut self, h: Arc<dyn SubHandle<ABClient = <T as SubHandle>::ABClient, Id = <T as SubHandle>::Id, Data = <T as SubHandle>::Data>>) {
        self.handlers.push(h);
    }

    fn handler_count(&self) -> usize {
        self.handlers.len()
    }

    fn get_handler(&self, i: usize) -> &dyn SubHandle<ABClient = <T as SubHandle>::ABClient, Id = <T as SubHandle>::Id, Data = <T as SubHandle>::Data> {
        self.handlers[i].as_ref()
    }

}
#[derive(Copy, Clone)]
pub struct TestHandler{

}

impl SubHandle for TestHandler  {
    type ABClient = AbClient;
    type Id = usize;
    type Data = Message<'_>;

    fn handle(&self, data: & Message<'_>, clients: &Arc<Mutex<HashMap<usize, Box<AbClient>>>>,id:usize) -> Option<Vec<u8>> {
        return Some(vec![b'{',b'"',b'r',b'e',b't',b'"',b':',b'0',b'}']);
    }
}

