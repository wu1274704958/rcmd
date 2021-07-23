use std::sync::Arc;
use crate::client_plugs::p2p_plugs::P2PPlug;
use crate::utils::command_mgr::{CmdHandler, CmdRet};
use async_trait::async_trait;
use std::any::{Any, TypeId};
use tokio::fs::OpenOptions;
use rcmd_suit::tools::{TOKEN_BEGIN, TOKEN_END, SEND_BUF_SIZE};
use tokio::io::AsyncReadExt;
use crate::extc::*;

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
        println!("{} 请求p2p连接是否同意？[y/n]",self.cp);
    }

    async fn on_pop(&mut self) {
        if self.accept {
            println!("已同意！");
        }else{
            println!("未同意！");
        }
    }


    async fn on_one_key(&mut self, b: u8) -> CmdRet {
        CmdRet::None
    }

    async fn on_line(&mut self, cmd: Vec<&str>, str: &str) -> CmdRet {
        self.accept = "y" == cmd[0];
        self.p2p_plug.accept_p2p(self.cp,self.accept).await;
        CmdRet::PoPSelf
    }

    fn get_type(&self) -> TypeId {
        self.type_id()
    }
}

pub struct P2PCmd{
    p2p_plug : Arc<P2PPlug>,
    cp: usize
}

impl P2PCmd{
    pub fn new(p2p_plug: Arc<P2PPlug>, cp: usize) -> Self {
        P2PCmd { p2p_plug, cp }
    }
}
#[async_trait]
impl CmdHandler for P2PCmd{
    async fn on_add(&mut self) {
        println!("into p2p cp {}",self.cp);
    }

    async fn on_pop(&mut self) {
        println!("bye bye");
    }

    async fn on_one_key(&mut self, b: u8) -> CmdRet {
        CmdRet::None
    }

    async fn on_line(&mut self, cmds: Vec<&str>, str: &str) -> CmdRet {
        match cmds[0] {
            "-" => { CmdRet::PoPSelf }
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
                                        self.p2p_plug.send_msg(self.cp,d,EXT_UPLOAD_FILE_ELF).await;
                                        break;
                                    } else {
                                        d.reserve(n);
                                        d.extend_from_slice(&buf[..]);
                                        self.p2p_plug.send_msg(self.cp,d, if is_first { EXT_UPLOAD_FILE_CREATE } else { EXT_UPLOAD_FILE }).await;
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
                CmdRet::None
            }
            _ => {
                CmdRet::None
            }
        }

    }

    fn get_type(&self) -> TypeId {
        self.type_id()
    }
}