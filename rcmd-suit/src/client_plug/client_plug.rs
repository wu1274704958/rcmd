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
pub trait ClientPlug:Send + Sync {
    type SockTy;
    type ErrTy;
    async fn on_init(&self);
    async fn on_create_socket(&self,sock:Arc<Self::SockTy>);
    async fn on_get_err(&self,err:Self::ErrTy) where Self::ErrTy :Clone;
    async fn on_begin(&self);
    async fn on_stop(&self);
}

pub struct ClientPluCollect<T> where T:ClientPlug {
    plugs : Vec<Arc<dyn ClientPlug<SockTy = <T as ClientPlug>::SockTy, ErrTy = <T as ClientPlug>::ErrTy>>>
}

impl<T> ClientPluCollect<T>  where T:ClientPlug {
    pub fn new()->ClientPluCollect<T>
    {
        ClientPluCollect::<T>{
            plugs:Vec::new()
        }
    }

    fn add_plug(&mut self,p:Arc<dyn ClientPlug<SockTy = <T as ClientPlug>::SockTy, ErrTy = <T as ClientPlug>::ErrTy>>)
    {
        self.plugs.push(p);
    }
    fn plug_count(&self)->usize
    {
        self.plugs.len()
    }
    fn get_plug(&self,i:usize)->&dyn ClientPlug<SockTy = <T as ClientPlug>::SockTy, ErrTy = <T as ClientPlug>::ErrTy>
    {
        self.plugs[i].as_ref()
    }

    pub async fn on_init(&self)
    {
        for i in 0..self.plug_count()
        {
            let plug = self.get_plug(i);
            plug.on_init().await;
        }
    }
    pub async fn on_create_socket(&self,sock:Arc<<T as ClientPlug>::SockTy>)
        where <T as ClientPlug>::SockTy : Send + Sync
    {
        for i in 0..self.plug_count()
        {
            let plug = self.get_plug(i);
            plug.on_create_socket(sock.clone()).await;
        }
    }
    pub async fn on_get_err(&self,err:<T as ClientPlug>::ErrTy)
        where <T as ClientPlug>::ErrTy: Clone + Send + Sync
    {
        for i in 0..self.plug_count()
        {
            let plug = self.get_plug(i);
            plug.on_get_err(err.clone()).await;
        }
    }
    pub async fn on_begin(&self)
    {
        for i in 0..self.plug_count()
        {
            let plug = self.get_plug(i);
            plug.on_begin().await;
        }
    }
    pub async fn on_stop(&self)
    {
        for i in 0..self.plug_count()
        {
            let plug = self.get_plug(i);
            plug.on_stop().await;
        }
    }
}
