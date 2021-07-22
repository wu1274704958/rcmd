use std::sync::Arc;
use crate::client_plugs::p2p_plugs::P2PPlug;
use crate::utils::command_mgr::{CmdHandler, CmdRet};
use async_trait::async_trait;
use std::any::{Any, TypeId};

pub struct AcceptP2P{
    p2p_plug : Arc<P2PPlug>,
    cp: usize,
    accept:bool
}

impl AcceptP2P{
    pub fn new(p2p_plug: Arc<P2PPlug>, cp: usize) -> Self {
        AcceptP2P { p2p_plug, cp ,accept:false}
    }
}
#[async_trait]
impl CmdHandler for AcceptP2P
{
    async fn on_add(&mut self) {
        print!("{} 请求p2p连接是否同意？[y/n]",self.cp);
    }

    async fn on_pop(&mut self) {
        if self.accept {
            println!("已同意！");
        }else{
            println!("未同意！");
        }
    }

    async fn is_handle_once(&self) -> bool {
        true
    }

    async fn on_one_key(&mut self, b: u8) -> CmdRet {
        self.accept = b == b'y';
        self.p2p_plug.accept_p2p(self.cp,self.accept).await;
        CmdRet::PoPSelf
    }

    async fn on_line(&mut self, cmd: Vec<&str>, str: &str) -> CmdRet {
        CmdRet::None
    }

    fn get_type(&self) -> TypeId {
        self.type_id()
    }
}