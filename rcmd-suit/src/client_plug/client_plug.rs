use std::sync::Arc;
use async_trait::async_trait;
use std::net::SocketAddr;

use crate::{agreement::Message};
#[async_trait]
pub trait ClientPlug:Send + Sync {
    type SockTy;
    type ErrTy;
    async fn on_init(&self);
    async fn on_create_socket(&self,sock:Arc<Self::SockTy>);
    async fn on_get_local_addr(&self,addr:SocketAddr);
    async fn on_get_err(&self,err:Self::ErrTy) where Self::ErrTy :Clone;
    async fn on_lauch_recv_worker(&self);
    async fn on_stop(&self);
    async fn on_recv_oth_msg(&self,addr:SocketAddr,data:&[u8]);
    async fn on_lauch_loop(&self);
    async fn handle(&self,_msg:Message<'_>) {}
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

    pub fn add_plug(&mut self,p:Arc<dyn ClientPlug<SockTy = <T as ClientPlug>::SockTy, ErrTy = <T as ClientPlug>::ErrTy>>)
    {
        self.plugs.push(p);
    }
    pub fn plug_count(&self)->usize
    {
        self.plugs.len()
    }
    pub fn get_plug(&self,i:usize)->&dyn ClientPlug<SockTy = <T as ClientPlug>::SockTy, ErrTy = <T as ClientPlug>::ErrTy>
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

    pub async fn on_get_local_addr(&self,addr:SocketAddr)
    {
        for i in 0..self.plug_count()
        {
            let plug = self.get_plug(i);
            plug.on_get_local_addr(addr).await;
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
    pub async fn on_lauch_recv_worker(&self)
    {
        for i in 0..self.plug_count()
        {
            let plug = self.get_plug(i);
            plug.on_lauch_recv_worker().await;
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

    pub async fn on_recv_oth_msg(&self,addr:SocketAddr,data:&[u8])
    {
        for i in 0..self.plug_count()
        {
            let plug = self.get_plug(i);
            plug.on_recv_oth_msg(addr,data).await;
        }
    }

    pub async fn on_lauch_loop(&self)
    {
        for i in 0..self.plug_count()
        {
            let plug = self.get_plug(i);
            plug.on_lauch_loop().await;
        }
    }

    pub async fn handle(&self,_msg:Message<'_>) {
        for i in 0..self.plug_count()
        {
            let plug = self.get_plug(i);
            plug.handle(_msg).await;
        }
    }

}
