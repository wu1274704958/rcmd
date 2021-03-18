use std::sync::Arc;
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
use tokio::sync::Mutex;
use async_trait::async_trait;

#[async_trait]
pub trait SubHandle:Send + Sync {
    type ABClient;
    type Id;
    async fn handle(&self,data:&[u8],len:u32,ext:u32,clients:&Arc<Mutex<HashMap<Self::Id,Box<Self::ABClient>>>>,id:Self::Id)-> Option<(Vec<u8>,u32)>
        where Self::Id :Copy;
    fn interested(&self,ext:u32)->bool{true}
}

#[async_trait]
pub trait Handle<T> where T:SubHandle
{
    async fn handle_ex(&self,data:Message<'_>,
                 clients:&Arc<Mutex<HashMap<<T as SubHandle>::Id,Box<<T as SubHandle>::ABClient>>>>,
                 id:<T as SubHandle>::Id)-> Option<(Vec<u8>,u32)>
        where <T as SubHandle>::Id:Copy + Send,
        <T as SubHandle>::ABClient : Send,T: 'async_trait
    {
        for i in 0..self.handler_count()
        {
            let handler = self.get_handler(i);
            if handler.interested(data.ext)
            {
                if let Some((v,ext)) = handler.handle(data.msg,data.len,data.ext,clients,id).await
                {
                    return Some((v,ext));
                }
            }
        }
        None
    }

    fn add_handler(&mut self,h:Arc<dyn SubHandle<ABClient = <T as SubHandle>::ABClient, Id = <T as SubHandle>::Id>>);
    fn handler_count(&self)->usize;
    fn get_handler(&self,i:usize)->&dyn SubHandle<ABClient = <T as SubHandle>::ABClient, Id = <T as SubHandle>::Id>;
}


pub struct DefHandler<T> where T:SubHandle {
    handlers : Vec<Arc<dyn SubHandle<ABClient = <T as SubHandle>::ABClient, Id = <T as SubHandle>::Id>>>
}

impl<T> DefHandler<T>  where T:SubHandle {
    pub fn new()->DefHandler<T>
    {
        DefHandler::<T>{
            handlers:Vec::new()
        }
    }
}
#[async_trait]
impl<T> Handle<T> for DefHandler<T> where T:SubHandle{

    fn add_handler(&mut self, h: Arc<dyn SubHandle<ABClient = <T as SubHandle>::ABClient, Id = <T as SubHandle>::Id>>) {
        self.handlers.push(h);
    }

    fn handler_count(&self) -> usize {
        self.handlers.len()
    }

    fn get_handler(&self, i: usize) -> &dyn SubHandle<ABClient = <T as SubHandle>::ABClient, Id = <T as SubHandle>::Id> {
        self.handlers[i].as_ref()
    }

}
#[derive(Copy, Clone)]
pub struct TestHandler{
}

#[async_trait]
impl  SubHandle for TestHandler  {
    type ABClient = AbClient;
    type Id = usize;

    async fn handle(&self, data: &[u8], len: u32, ext: u32,
              clients: &Arc<Mutex<HashMap<Self::Id, Box<Self::ABClient>>>>,
              id: Self::Id) -> Option<(Vec<u8>,u32)> where Self::Id: Copy {
        let mut a = clients.lock().await;
        if let Some(c) = a.get_mut(&id)
        {
            c.heartbeat_time = SystemTime::now();
        }
        return None;
    }

}

