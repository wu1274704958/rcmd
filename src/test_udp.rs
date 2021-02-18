#![allow(unused_imports)]
mod handlers;
mod extc;
mod model;
#[macro_use]
extern crate rcmd_suit;
#[macro_use]
extern crate lazy_static;
mod comm;

use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;
use std::sync::{Arc};
use tokio::sync::Mutex;
use std::cell::RefCell;
use std::ops::AddAssign;
use std::borrow::BorrowMut;
use std::collections::HashMap;
use tokio::time::Duration;
use std::env;
use std::net::Ipv4Addr;
use std::str::FromStr;
use std::time::SystemTime;
use getopts::HasArg::No;

use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use rcmd_suit::ab_client::ABClient;
use std::hash::BuildHasher;
use ahash::RandomState;
use ahash::CallHasher;
use std::env::consts::OS;
use rcmd_suit::config_build::ConfigBuilder;
use rcmd_suit::tools;
use rcmd_suit::handler::{DefHandler, TestHandler, Handle};
use rcmd_suit::agreement::DefParser;
use rcmd_suit::plug::{DefPlugMgr, PlugMgr};
use rcmd_suit::plugs::heart_beat::HeartBeat;
use rcmd_suit::db::db_mgr::DBMgr;
use rcmd_suit::utils::temp_permission::TempPermission;
use rcmd_suit::servers::udp_server::UdpServer;
use rcmd_suit::tools::platform_handle;
use rcmd_suit::servers::udp_server::run_in;
use rcmd_suit::utils::udp_sender::{DefUdpSender, UdpSender};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>>{

    let config = ConfigBuilder::new()
        .thread_count(8)
        .build();

    let config = tools::parse_args(config);

    let mut handler = DefHandler::<TestHandler>::new();
    let mut parser = DefParser::new();
    let mut plugs = DefPlugMgr::<HeartBeat>::new();
    let mut dead_plugs = DefPlugMgr::<HeartBeat>::new();

    let dbmgr = Arc::new(DBMgr::new().unwrap());
    let user_map = Arc::new(Mutex::new(HashMap::<usize,model::user::User>::new()));
    let login_map = Arc::new(Mutex::new(HashMap::<String,usize>::new()));
    let temp_permission = TempPermission::new();

    {
        handler.add_handler(Arc::new(handlers::heart_beat::HeartbeatHandler{}));
        handler.add_handler(Arc::new(TestHandler{}));
        handler.add_handler(Arc::new(handlers::upload_file::UploadHandler::new(user_map.clone())));
        handler.add_handler(Arc::new(handlers::login::Login::new(dbmgr.clone(),user_map.clone(),login_map.clone())));
        //parser.add_transform(Arc::new(DefCompress{}));
        handler.add_handler(Arc::new(handlers::register::Register::new(dbmgr.clone(),user_map.clone())));
        handler.add_handler(Arc::new(handlers::get_users::GetUser::new(user_map.clone(),login_map.clone())));
        handler.add_handler(Arc::new(handlers::send_msg::SendMsg::new(user_map.clone(),login_map.clone())));
        handler.add_handler(Arc::new(handlers::exec_cmd::ExecCmd::new(user_map.clone())));
        handler.add_handler(Arc::new(handlers::send_file::SendFile::new(user_map.clone(),temp_permission.clone())));
        handler.add_handler(Arc::new(handlers::pull_file::PullFile::new(user_map.clone(),temp_permission.clone())));

        plugs.add_plug(Arc::new(HeartBeat{}));

        dead_plugs.add_plug(Arc::new(handlers::login::OnDeadPlug::new(
            dbmgr.clone(),
            user_map.clone(),
            login_map.clone(),
            temp_permission.clone())));
    }

    let server = UdpServer::new(
        handler.into(),
        parser.into(),
        plugs.into(),
        dead_plugs.into(),
        config
    );
    lazy_static::initialize(&comm::IGNORE_EXT);
    let msg_split_ignore:Option<&Vec<u32>> = Some(&comm::IGNORE_EXT);
    let asy_cry_ignore:Option<&Vec<u32>> = Some(&comm::IGNORE_EXT);
    //udp_server_run!(server,msg_split_ignore,msg_split_ignore);

    let channel_buf = server.channel_buf;
    let sock = Arc::new(UdpSocket::bind(server.config.addr).await?);
    platform_handle(sock.as_ref());

    let linker_map = Arc::new(std::sync::Mutex::new(HashMap::<u64,Arc<DefUdpSender>>::new()));
    let mut hash_builder = RandomState::new();

    loop {
        let mut buf = Vec::with_capacity(server.buf_len);
        buf.resize(server.buf_len,0);
        match sock.recv_from(&mut buf[..]).await
        {
            Ok((len,addr)) => {

                let id = CallHasher::get_hash(&addr, hash_builder.build_hasher());
                let has = {
                    let mut map = linker_map.lock().unwrap();
                    if let Some(link) = map.get(&id){
                        link.check_recv(&buf[0..len]).await;
                        while link.need_check().await { link.check_recv(&[]).await; }
                        true
                    }else { false }
                };
                if !has
                {
                    let mut sender = Arc::new(DefUdpSender::create(sock.clone(),addr));
                    {
                        sender.check_recv(&buf[0..len]).await;
                        println!("check_recv end -------------------------");
                        while sender.need_check().await { sender.check_recv(&[]).await; }
                        println!("-------------------------");
                        let mut map = linker_map.lock().unwrap();
                        map.insert(id, sender.clone());
                    }
                    {
                        let linker_map_cp = linker_map.clone();
                        let clients = server.clients.clone();
                        let lid = server.logic_id.clone();
                        let conf = server.config.clone();
                        let handler_cp = server.handler.clone();
                        let parser_cp = server.parser.clone();
                        let plugs_cp = server.plug_mgr.clone();
                        let dead_plugs_cp:Arc<_> = server.dead_plug_mgr.clone();
                        let sock_cp = sock.clone();
                        dbg!(&addr);

                        server.runtime.spawn(run_in(
                        clients,lid,conf,handler_cp,parser_cp,plugs_cp,dead_plugs_cp,sock_cp,sender,
                        addr,move ||{
                                let id_ = id;
                                let mut map = linker_map_cp.lock().unwrap();
                                map.remove(&id_);
                                println!("disconnect id = {}",id_);
                            },asy_cry_ignore,msg_split_ignore
                        ));
                    }
                }
            }
            Err(e) => {
                eprintln!("err = {:?}",e);
            }
        }
    }

    Ok(())
}