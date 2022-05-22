use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::VecDeque;
use crate::client_plugs::p2p_plugs::P2PPlug;
use crate::utils::command_mgr::{CmdHandler, CmdRet};
use tokio::fs::OpenOptions;
use rcmd_suit::tools::{TOKEN_BEGIN, TOKEN_END, SEND_BUF_SIZE};
use tokio::io::AsyncReadExt;
use crate::model;
use std::str::FromStr;
use crate::extc::*;
use async_trait::async_trait;
use crate::command::remote_cmd::RemoteCmd;
use std::any::{TypeId, Any};
use crate::command::p2p_cmd::P2PCmd;
use rcmd_suit::clients::udp_client::{UdpClient, UdpClientErr};
use rcmd_suit::utils::udp_sender::DefUdpSender;
use rcmd_suit::agreement::DefParser;
use rcmd_suit::client_handler::DefHandler;

pub struct MainCmd{
    client: Arc<UdpClient<DefHandler,DefParser,DefUdpSender>>,
    p2p_plug: Arc<P2PPlug>
}

impl MainCmd{
    pub fn new(client: Arc<UdpClient<DefHandler,DefParser,DefUdpSender>>, p2p_plug: Arc<P2PPlug>) -> Self {
        MainCmd { client, p2p_plug }
    }

    pub async fn send(&self,data: Vec<u8>,ext:u32)
    {
        self.client.send(data,ext).await;
    }
}
#[async_trait]
impl CmdHandler for MainCmd{
    async fn on_add(&mut self) {
        println!("欢迎使用！！！傻妞O(∩_∩)O为您服务。");
    }

    async fn on_pop(&mut self) {
        if let Err(UdpClientErr::UsError(e)) = self.client.close_session().await{
            eprintln!("close session failed {:?}",e);
        };
        println!("再见！(*^_^*)");
    }

    async fn on_one_key(&mut self, b: u8) -> CmdRet {
        CmdRet::None
    }

    async fn on_line(&mut self, cmds: Vec<&str>,str:&str) -> CmdRet {
        match cmds[0] {
            "-" => {
                return CmdRet::PoPSelf;
            }
            "0" => {
                if cmds.len() >= 2 {
                    self.send(cmds[1].into(), 0).await;
                }
            },
            "1" => {
                if cmds.len() < 3 { return CmdRet::None;}
                match OpenOptions::new().read(true).open(cmds[1]).await
                {
                    Ok(mut f) => {
                        let mut head_v = vec![];
                        head_v.push(TOKEN_BEGIN);
                        cmds[2].trim().as_bytes().iter().for_each(|it| { head_v.push(*it) });
                        head_v.push(TOKEN_END);

                        let mut buf = Vec::with_capacity(SEND_BUF_SIZE);
                        buf.resize(SEND_BUF_SIZE, 0);
                        let mut is_first = true;
                        loop {
                            let mut d = head_v.clone();
                            match f.read(&mut buf[..]).await {
                                Ok(n) => {
                                    //println!("==== {} ====",n);
                                    if n <= 0
                                    {
                                        self.send( d, EXT_UPLOAD_FILE_ELF).await;
                                        break;
                                    } else {
                                        d.reserve(n);
                                        for i in 0..n { d.push(buf[i]); }
                                        self.send(  d, if is_first { EXT_UPLOAD_FILE_CREATE } else { EXT_UPLOAD_FILE }).await;
                                        is_first = false;
                                    }
                                }
                                _ => {}
                            }
                        }
                        //println!("==== end ====");
                    }
                    Err(e) => {
                        eprintln!("{}", e);
                    }
                }
            }
            "2" => {
                if cmds.len() < 3 { return CmdRet::None; }
                let acc = cmds[1].trim().to_string();
                let pwd = cmds[2].trim().to_string();
                let user = model::user::MinUser { acc, pwd };
                let s = serde_json::to_string(&user).unwrap();
                self.send( s.into_bytes(), EXT_LOGIN).await;
            }
            "3" => {
                self.send( vec![], EXT_LOGOUT).await;
            }
            "4" => {
                if cmds.len() < 4 { return CmdRet::None; }
                let acc = cmds[1].trim().to_string();
                let pwd = cmds[2].trim().to_string();
                let name = cmds[3].trim().to_string();
                let user = model::user::RegUser { acc, pwd, name };
                let s = serde_json::to_string(&user).unwrap();
                self.send( s.into_bytes(), EXT_REGISTER).await;
            }
            "5" => {
                self.send( vec![], EXT_GET_USERS).await;
            }
            "6" => {
                if cmds.len() < 3 { return CmdRet::None; }
                let lid = match usize::from_str(cmds[1]) {
                    Ok(v) => { v }
                    Err(e) => {
                        dbg!(e);
                        return CmdRet::None;
                    }
                };
                let msg = cmds[2].trim().to_string();
                let su = model::SendMsg { lid, msg };
                self.send( serde_json::to_string(&su).unwrap().into_bytes(), EXT_SEND_MSG).await;
            }
            "7" => {
                if cmds.len() < 2 { return CmdRet::None; }
                let msg = cmds[1].trim().to_string();
                self.send( msg.into_bytes(), EXT_SEND_BROADCAST).await;
            }
            "8" => {
                if cmds.len() < 2 { return CmdRet::None; }
                let lid = match usize::from_str(cmds[1].trim()) {
                    Ok(v) => { v }
                    Err(e) => {
                        dbg!(e);
                        return CmdRet::None;
                    }
                };
                return CmdRet::Push(Box::new(RemoteCmd::new(self.client.clone(),lid)));
            }
            "9" => {
                if cmds.len() < 2 { return CmdRet::None; }
                let lid = match usize::from_str(cmds[1].trim()) {
                    Ok(v) => { v }
                    Err(e) => {
                        dbg!(e);
                        return CmdRet::None;
                    }
                };
                self.p2p_plug.req_link(lid).await;
            }
            "p" => {
                if cmds.len() < 2 { return CmdRet::None; }
                let lid = match usize::from_str(cmds[1].trim()) {
                    Ok(v) => { v }
                    Err(e) => {
                        dbg!(e);
                        return CmdRet::None;
                    }
                };
                if self.p2p_plug.has_entity(lid).await
                {
                    return CmdRet::Push(Box::new(P2PCmd::new(self.p2p_plug.clone(),lid)));
                }else{
                    println!("Not find this entity!");
                }
                return CmdRet::None;
            }
            _ => {
                let help = r"
                    1 upload file
                    2 login [acc pwd]
                    3 logout
                    4 register [acc pwd name]
                    5 user list
                    6 send_msg [lid msg]
                    7 broadcast [msg]
                ";
                println!("{}", help);
            }
        }
        CmdRet::None
    }

    fn get_type(&self) -> TypeId {
        self.type_id()
    }
}

