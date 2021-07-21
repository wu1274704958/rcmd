use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::VecDeque;

pub enum CmdRet
{
    None,
    PoPSelf,
}

#[async_trait]
pub trait CmdHandler : std::marker::Send +  std::marker::Sync
{
    async fn on_add(&self);
    async fn on_pop(&self);
    async fn is_handle_once(&self) -> bool { false }
    async fn on_one_key(&self,b:u8) -> CmdRet;
    async fn on_line(&self,cmd:Vec<&str>) -> CmdRet;
}

pub struct CmdMgr<CH>
    where CH:CmdHandler
{
    handler: Arc<Mutex<Stack<CH>>>,
}
