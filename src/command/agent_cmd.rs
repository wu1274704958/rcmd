use std::sync::Arc;
use crate::utils::command_mgr::{CmdHandler, CmdRet};
use crate::extc::*;
use async_trait::async_trait;
use std::any::{TypeId, Any};
use rcmd_suit::clients::udp_client::{UdpClient, IUdpClient};
use rcmd_suit::utils::udp_sender::DefUdpSender;
use rcmd_suit::agreement::DefParser;
use rcmd_suit::client_handler::DefHandler;

pub struct AgentCmd{
    cp:usize,
    client: Arc<UdpClient<DefHandler,DefParser,DefUdpSender>>
}

impl AgentCmd{
    pub fn new(client: Arc<UdpClient<DefHandler,DefParser,DefUdpSender>>,cp:usize) -> Self {
        AgentCmd { client ,cp}
    }

    pub async fn send(&self,data: Vec<u8>,ext:u32)
    {
        self.client.send(data,ext).await;
    }
    fn new_data(&self) -> Vec<u8>
    {
        let mut d = Vec::<u8>::new();
        let slice = self.cp.to_be_bytes();
        d.extend_from_slice(slice.as_slice());
        d
    }
    fn new_data_with(&self,after:Vec<u8>) -> Vec<u8>
    {
        let mut d = Vec::<u8>::new();
        let slice = self.cp.to_be_bytes();
        d.extend_from_slice(slice.as_slice());
        d.extend_from_slice(after.as_slice());
        d
    }
}
#[async_trait]
impl CmdHandler for AgentCmd{
    async fn on_add(&mut self) {
        println!("欢迎使用！！！傻妞O(∩_∩)O为您服务。");
    }

    async fn on_pop(&mut self) {
        println!("再见！(*^_^*)");
    }

    async fn on_one_key(&mut self, _b: u8) -> CmdRet {
        CmdRet::None
    }

    async fn on_line(&mut self, cmds: Vec<&str>,_str:&str) -> CmdRet {
        match cmds[0] {
            "-" => {
                return CmdRet::PoPSelf;
            }
            "data" => {
                self.send(self.new_data(), EXT_AGENT_GET_DATA_CS).await;
            },
            "exec" => {
                if cmds.len() < 2 { return CmdRet::None;}
                let cmd = cmds[1].trim().to_string();
                self.send(self.new_data_with(cmd.into_bytes()), EXT_AGENT_EXEC_CMD_CS).await;
            }
            "set" => {
                if cmds.len() < 3 { return CmdRet::None; }
                let mut args = String::new();
                for c in 1..cmds.len(){
                    args.push_str(cmds[c]);
                }
                
                self.send(self.new_data_with(args.into_bytes()), EXT_AGENT_SET_DATA_CS).await;
            }
            _ => {
            }
        }
        CmdRet::None
    }

    fn get_type(&self) -> TypeId {
        self.type_id()
    }
}

