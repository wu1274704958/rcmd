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
use std::any::{TypeId, Any};
use rcmd_suit::clients::udp_client::{UdpClient, IUdpClient};
use rcmd_suit::client_handler::DefHandler;
use rcmd_suit::agreement::DefParser;
use rcmd_suit::utils::udp_sender::DefUdpSender;

pub struct RemoteCmd{
    client: Arc<UdpClient<DefHandler,DefParser,DefUdpSender>>,
    remote_id:usize
}

impl RemoteCmd{

    pub async fn send(&self,data: Vec<u8>,ext:u32)
    {
        self.client.send(data,ext).await;
    }

    pub fn new(client: Arc<UdpClient<DefHandler,DefParser,DefUdpSender>>, remote_id: usize) -> Self {
        RemoteCmd { client, remote_id }
    }
}
#[async_trait]
impl CmdHandler for RemoteCmd {
    async fn on_add(&mut self) {
        println!("欢迎使用！！！小媛O(∩_∩)O为您服务。");
    }

    async fn on_pop(&mut self) {
        println!("再见！(*^_^*)");
    }

    async fn on_one_key(&mut self, b: u8) -> CmdRet {
        CmdRet::None
    }

    async fn on_line(&mut self, cmds: Vec<&str>,str:&str) -> CmdRet {
        match cmds[0] {
            "-" => {
                return CmdRet::PoPSelf;
            },
            "#" => {
                match cmds[1] {
                    "1" => {
                        if cmds.len() < 3 { return CmdRet::None; }
                        match OpenOptions::new().read(true).open(cmds[2]).await
                        {
                            Ok(mut f) => {
                                let mut head_v = self.remote_id.to_be_bytes().to_vec();
                                head_v.push(TOKEN_BEGIN);
                                head_v.extend_from_slice(cmds[2].as_bytes());
                                head_v.push(TOKEN_END);

                                let mut buf = Vec::with_capacity(SEND_BUF_SIZE);
                                buf.resize(SEND_BUF_SIZE, 0);
                                let mut is_first = true;
                                let mut bytes = 0;
                                loop {
                                    let mut d = head_v.clone();
                                    match f.read(&mut buf[..]).await {
                                        Ok(n) => {
                                            //println!("==== {} ====",n);
                                            if n <= 0
                                            {
                                                self.send( d, EXT_SEND_FILE_ELF).await;
                                                println!("file size {}", bytes);
                                                break;
                                            } else {
                                                d.reserve(n);
                                                d.extend_from_slice(&buf[0..n]);
                                                bytes += n;
                                                self.send( d, if is_first { EXT_SEND_FILE_CREATE } else { EXT_SEND_FILE }).await;
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
                                return CmdRet::None;
                            }
                        }
                    }
                    "2" => {
                        if cmds.len() < 2 { return CmdRet::None; }
                        let mut head_v = self.remote_id.to_be_bytes().to_vec();
                        let pull_msg = model::PullFileMsg {
                            far_end_path: cmds[2].to_string(),
                            near_end_path: if cmds.len() >= 3 {
                                Some(cmds[2].trim().to_string())
                            } else { None }
                        };
                        let s = serde_json::to_string(&pull_msg).unwrap();
                        head_v.extend_from_slice(s.as_bytes());
                        self.send( head_v, EXT_PULL_FILE_S).await;
                    }
                    _ => {}
                }
            }
            _ => {
                let su = model::SendMsg { lid:self.remote_id, msg: str.to_string() };
                self.send( serde_json::to_string(&su).unwrap().into_bytes(), EXT_RUN_CMD).await;
            }
        }
        CmdRet::None
    }

    fn get_type(&self) -> TypeId {
        self.type_id()
    }
}