use std::sync::Arc;
use std::sync::Mutex;
use std::collections::HashMap;
use crate::ab_client::AbClient;
use std::collections::hash_map::RandomState;
use crate::agreement::Message;
use std::future::Future;
use async_std::task::Context;
use tokio::macros::support::{Pin, Poll};


pub trait SubHandle<'a,ABClient,Id,Data> :Send + Sync {
    fn handle(&self,data:&'a Data,clients:&'a Arc<Mutex<HashMap<Id,Box<ABClient>>>>,id:Id)-> Option<Vec<u8>> where Id:Copy;
}

pub trait Handle<'a>
{
    fn handle_ex(&self,data:&'a Message<'a>,clients:&'a Arc<Mutex<HashMap<usize,Box<AbClient>>>>,id:usize)-> Option<Vec<u8>>
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

    fn add_handler(&mut self,h:Arc<dyn SubHandle<'a,AbClient,usize,Message<'a>>>);
    fn handler_count(&self)->usize;
    fn get_handler(&self,i:usize)->&dyn SubHandle<'a,AbClient,usize,Message<'a>>;
}

#[derive(Clone)]
pub struct DefHandler<'a>{
    handlers : Vec<Arc<dyn SubHandle<'a,AbClient,usize,Message<'a>>>>
}

impl<'a> DefHandler<'a>   {
    pub fn new()->DefHandler<'a>
    {
        DefHandler::<'a>{
            handlers:Vec::new()
        }
    }
}

impl <'a> Handle<'a> for DefHandler<'a> {

    fn add_handler(&mut self, h: Arc<dyn SubHandle<'a, AbClient, usize, Message<'a>>>) {
        self.handlers.push(h);
    }

    fn handler_count(&self) -> usize {
        self.handlers.len()
    }

    fn get_handler(&self, i: usize) -> &dyn SubHandle<'a, AbClient, usize, Message<'a>> {
        self.handlers[i].as_ref()
    }

}
#[derive(Copy, Clone)]
pub struct TestHandler{

}

impl<'a> SubHandle<'a,AbClient,usize,Message<'a>> for TestHandler  {

    fn handle(&self, data: &'a Message<'a>, clients: &'a Arc<Mutex<HashMap<usize, Box<AbClient>>>>,id:usize) -> Option<Vec<u8>> {
        return Some(vec![b'{',b'"',b'r',b'e',b't',b'"',b':',b'0',b'}']);
    }
}

