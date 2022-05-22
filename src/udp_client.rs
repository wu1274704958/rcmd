use tokio::{io,sync::Mutex};
use tokio::prelude::*;
mod extc;
mod model;
mod client_handlers;
mod client_plugs;
mod handlers;
mod db;
mod utils;
mod command;
mod p2p_client_handler;
#[macro_use]
extern crate lazy_static;
extern crate get_if_addrs;
mod comm;

use std::sync::{Arc};
use std::str::FromStr;


use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::*;
use extc::*;
use rcmd_suit::tools;
use rcmd_suit::clients::udp_client::UdpClient;
use std::net::{ IpAddr};
use rcmd_suit::agreement::DefParser;
use rcmd_suit::client_handler::{DefHandler, Handle};
use rcmd_suit::tools::{TOKEN_BEGIN, TOKEN_END, SEND_BUF_SIZE};
use rcmd_suit::utils::udp_sender::{DefUdpSender, USErr};
use rcmd_suit::client_plug::client_plug::ClientPluCollect;
use crate::client_plugs::p2p_plugs::P2PPlug;
use crate::utils::command_mgr::CmdMgr;
use crate::command::main_cmd::MainCmd;
use futures::executor::block_on;
use crate::command::p2p_cmd::AcceptP2P;
use crate::client_plugs::p2p_event::P2PEvent;
use async_trait::async_trait;
use std::any::TypeId;

#[tokio::main]
async fn main() -> io::Result<()>
{
    let args = match tools::parse_c_args()
    {
        Ok(a) => {a}
        Err(e) => {
            dbg!(e);
            return Ok(());
        }
    };
    let ip = IpAddr::from_str("0.0.0.0").unwrap();

    let msg_queue = Arc::new(Mutex::new(VecDeque::<(Vec<u8>, u32)>::new()));
    if args.acc.is_some(){
        let user = model::user::MinUser{ acc: args.acc.unwrap(),pwd:args.pwd.unwrap()};
        let s = serde_json::to_string(&user).unwrap();
        send(&msg_queue,s.into_bytes(),EXT_LOGIN).await;
    }
    let is_runing = Arc::new(Mutex::new(true));
    let mut handler = DefHandler::new();

    let cmd_mgr = Arc::new(CmdMgr::new(is_runing.clone()));

    {
        handler.add_handler(Arc::new(client_handlers::get_users::GetUser::new()));
        handler.add_handler(Arc::new(client_handlers::get_users::RecvMsg::new()));
        handler.add_handler(Arc::new(client_handlers::err::Err{}));
        handler.add_handler(Arc::new(client_handlers::exec_cmd::Exec::new()));
        handler.add_handler(Arc::new(client_handlers::run_cmd::RunCmd::new()));
        handler.add_handler(Arc::new(client_handlers::send_file::SendFile::new()));
        handler.add_handler(Arc::new(client_handlers::save_file::SaveFile::with_observer(Box::new(on_save_file))));
        handler.add_handler(Arc::new(client_handlers::pull_file_ret::PullFileRet::new()));
    }

    let p2p_plug_clone = Arc::new(Mutex::new(None));

    let mut plugs = ClientPluCollect::<P2PPlug>::new();
    let p2p_plug = Arc::new(
        P2PPlug::new(msg_queue.clone(),
                     Some(Arc::new(DefP2PEvent::new(cmd_mgr.clone(),p2p_plug_clone.clone())))));
    plugs.add_plug(p2p_plug.clone());

    {
        let mut p = p2p_plug_clone.lock().await;
        *p = Some(p2p_plug.clone());
    }



    let client_plug_ptr = Arc::new(plugs);
    {
        let msg_queue = msg_queue.clone();
        let client = Arc::new( UdpClient::<_,_,DefUdpSender>::with_msg_queue_runing(
            (ip, args.bind_port),
            Arc::new(handler),
            DefParser::new(),
            msg_queue.clone(),
            is_runing
        ));
        lazy_static::initialize(&comm::IGNORE_EXT);
        let msg_split_ignore:Option<&Vec<u32>> = Some(&comm::IGNORE_EXT);
        let run = client.run::<P2PPlug,_>(
            args.ip,args.port,
            msg_split_ignore,msg_split_ignore,
            client_plug_ptr,async{});

        cmd_mgr.push(Box::new(MainCmd::new(client.clone(),p2p_plug.clone()))).await;

        let cmd_run = cmd_mgr.run();

        futures::join!(cmd_run,run);
    }
    Ok(())
}


async fn send(queue: &Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>, data: Vec<u8>,ext:u32) {
    let mut a = queue.lock().await;
    {
        a.push_back((data,ext));
    }
}
fn on_save_file(name:&str,len:usize,ext:u32)
{
    match ext {
        EXT_SAVE_FILE|
        EXT_SAVE_FILE_CREATE => {
            println!("recv file {} {} bytes!",name,len);
        }
        EXT_SAVE_FILE_ELF => {
            println!("recv file {} complete size {} bytes!",name,len);
        }
        _=>{}
    }
}

struct DefP2PEvent{
    mgr :Arc<CmdMgr>,
    p2p_plug:Arc<Mutex<Option<Arc<P2PPlug>>>>
}

impl DefP2PEvent{
    pub fn new(mgr: Arc<CmdMgr>, p2p_plug: Arc<Mutex<Option<Arc<P2PPlug>>>>) -> Self {
        DefP2PEvent { mgr, p2p_plug }
    }
}
#[async_trait]
impl P2PEvent for DefP2PEvent{
    async fn on_recv_p2p_req(&self, cpid: usize) {
        let plug = self.p2p_plug.lock().await;
        if let Some(ref p) = *plug{
            self.mgr.push(Box::new(AcceptP2P::new(p.clone(),cpid))).await;
        }
    }

    async fn on_recv_wait_accept_timeout(&self, cpid: usize) {
        if let Some(id) = self.mgr.get_top_type().await{
            if id == TypeId::of::<AcceptP2P>()
            {
                self.mgr.pop().await;
            }
        }
    }
}
