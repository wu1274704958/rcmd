use std::sync::Arc;
use std::sync::Mutex;
use std::collections::HashMap;
use crate::ab_client::AbClient;
use std::collections::hash_map::RandomState;
use crate::agreement::Message;
use std::future::Future;
use async_std::task::Context;
use tokio::macros::support::{Pin, Poll};
use std::marker::PhantomData;
use std::ops::{DerefMut, Deref};
use std::time::SystemTime;


pub trait SubHandle:Send + Sync {
    fn handle(&self,data:&[u8],len:u32,ext:u32)-> Option<(Vec<u8>,u32)>;
}

pub trait Handle
{
    fn handle_ex(&self,data:Message<'_>
                 )-> Option<(Vec<u8>,u32)>
    {
        for i in 0..self.handler_count()
        {
            let handler = self.get_handler(i);
            if let Some((v,ext)) = handler.handle(data.msg,data.len,data.ext)
            {
                return Some((v,ext));
            }
        }
        None
    }

    fn add_handler(&mut self,h:Arc<dyn SubHandle>);
    fn handler_count(&self)->usize;
    fn get_handler(&self,i:usize)->&dyn SubHandle;
}


pub struct DefHandler{
    handlers : Vec<Arc<dyn SubHandle>>
}

impl DefHandler {
    pub fn new()->DefHandler
    {
        DefHandler{
            handlers:Vec::new()
        }
    }
}

impl Handle for DefHandler{

    fn add_handler(&mut self, h: Arc<dyn SubHandle>) {
        self.handlers.push(h);
    }

    fn handler_count(&self) -> usize {
        self.handlers.len()
    }

    fn get_handler(&self, i: usize) -> &dyn SubHandle {
        self.handlers[i].as_ref()
    }

}


