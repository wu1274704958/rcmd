use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::VecDeque;
use tokio::io::AsyncReadExt;
use std::any::TypeId;


#[async_trait]
pub trait CmdHandler : std::marker::Send +  std::marker::Sync
{
    async fn on_add(&mut self);
    async fn on_pop(&mut self);
    async fn is_handle_once(&self) -> bool { false }
    async fn on_one_key(&mut self,b:u8) -> CmdRet;
    async fn on_line(&mut self,cmd:Vec<&str>,str:&str) -> CmdRet;
    fn get_type(&self) -> TypeId;
}

pub enum CmdRet
{
    None,
    PoPSelf,
    Push(Box<dyn CmdHandler>)
}

pub struct CmdMgr
{
    handler: Arc<Mutex<VecDeque<Box<dyn CmdHandler>>>>,
    is_runing: Arc<Mutex<bool>>,
}

impl CmdMgr
{
    pub fn new(is_runing: Arc<Mutex<bool>>) -> CmdMgr
    {
        CmdMgr{
            handler: Arc::new(Default::default()),
            is_runing
        }
    }

    pub async fn push(&self,mut handle:Box<dyn CmdHandler>)
    {
        handle.on_add().await;
        let mut h= self.handler.lock().await;
        h.push_back(handle);
    }

    pub async fn pop(&self) -> Option<Box<dyn CmdHandler>>
    {
        let mut h = self.handler.lock().await;
        if let Some(mut ch) = h.pop_back()
        {
            ch.on_pop().await;
            Some(ch)
        }else{
            None
        }
    }

    async fn on_key(&self,k:u8) -> bool
    {
        let mut pop = false;
        let mut push = None;
        let mut only_one = false;
        let mut h = self.handler.lock().await;
        only_one = h.len() == 1;
        if let Some(ch) = h.back_mut()
        {
            if ch.is_handle_once().await{
                match ch.on_one_key(k).await
                {
                    CmdRet::None => {}
                    CmdRet::PoPSelf => {
                        pop = true;
                    }
                    CmdRet::Push(ch) => {
                        push = Some(ch);
                    }
                }
            }
        }else{
            return false;
        }
        drop(h);
        if pop {
            if self.pop().await.is_some() && only_one{
                return false;
            }
        }
        if let Some(ch) = push{
            self.push(ch).await;
        }
        true
    }

    async fn on_line(&self,cmd:Vec<&str>,str:&str) -> bool
    {
        let mut pop = false;
        let mut push = None;
        let mut only_one = false;
        let mut h = self.handler.lock().await;
        only_one = h.len() == 1;
        if let Some(ch) = h.back_mut()
        {
            match ch.on_line(cmd,str).await
            {
                CmdRet::None => {}
                CmdRet::PoPSelf => {
                    pop = true;
                }
                CmdRet::Push(ch) => {
                    push = Some(ch);
                }
            }
        }else{
            return false;
        }
        drop(h);
        if pop {
            if self.pop().await.is_some() && only_one{
                return false;
            }
        }
        if let Some(ch) = push{
            self.push(ch).await;
        }
        true
    }

    pub async fn run(&self)
    {
        'Out: loop {
            let mut cmd = String::new();
            let mut vec = vec![];
            let mut in_ = tokio::io::stdin();
            loop {
                if let Ok(c) = in_.read_u8().await {
                    if c != b'\n'
                    {
                        if !self.on_key(c).await
                        {
                            break 'Out;
                        }
                        vec.push(c);
                    } else { break; }
                }
            }

            cmd = String::from_utf8_lossy(vec.as_slice()).trim().to_string();
            let cmds: Vec<&str> = cmd.split(" ").map(|it| { it.trim() }).collect();

            if !self.on_line(cmds,cmd.as_str()).await
            {
                break;
            }
        }
        let mut is_runing = self.is_runing.lock().await;
        *is_runing = false;
    }

    pub async fn get_top_type(&self) -> Option<TypeId>
    {
        let h = self.handler.lock().await;
        if let Some(ch) = h.back()
        {
            Some(ch.get_type())
        }else{
            None
        }
    }
}
