#![allow(unused_imports)]
mod config_build;
mod ab_client;
mod handler;
mod utils;
mod agreement;
mod plug;
mod plugs;
mod handlers;
mod asy_cry;
mod data_transform;
mod ext_code;
mod subpackage;
mod db;
mod model;
mod tools;
#[macro_use]
mod servers;

use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;
use std::thread::{ThreadId, Thread};
use crate::config_build::{ConfigBuilder};
use crate::ab_client::{AbClient, State};
use std::sync::{Arc};
use tokio::sync::Mutex;
use std::cell::RefCell;
use std::ops::AddAssign;
use crate::ab_client::State::{Dead, Busy, Ready, WaitKill};
use std::borrow::BorrowMut;
use std::collections::HashMap;
use tokio::time::Duration;
use crate::handler::{Handle, TestHandler, DefHandler};
use crate::tools::{read_form_buf, set_client_st, del_client, get_client_write_buf, handle_request, TOKEN_BEGIN, TOKEN_END, get_client_st};
use crate::agreement::{DefParser, Agreement,TestDataTransform,Test2DataTransform};
use crate::plug::{DefPlugMgr, PlugMgr};
use crate::plugs::heart_beat::HeartBeat;
use std::env;
use std::net::Ipv4Addr;
use std::str::FromStr;
use crate::asy_cry::{DefAsyCry, AsyCry, EncryptRes};
use crate::data_transform::def_compress::DefCompress;
use crate::subpackage::{DefSubpackage, Subpackage};
use std::time::SystemTime;
use crate::db::db_mgr::DBMgr;
use crate::utils::msg_split::{DefMsgSplit, MsgSplit};
use getopts::HasArg::No;
use crate::utils::temp_permission::TempPermission;
use crate::servers::udp_server::{UdpServer,run_in};

use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use ab_client::ABClient;
use std::hash::BuildHasher;
use ahash::RandomState;
use ahash::CallHasher;

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
        //handler.add_handler(Arc::new(TestHandler{}));
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

    let ser = UdpServer::new(
        handler.into(),
        parser.into(),
        plugs.into(),
        dead_plugs.into(),
        config
    );

    let channel_buf = ser.channel_buf;
    let sock = Arc::new(UdpSocket::bind(ser.config.addr).await?);

    let linker_map = Arc::new(Mutex::new(HashMap::<u64,mpsc::Sender<Vec<u8>>>::new()));
    let mut hash_builder = RandomState::new();

    loop {
        let mut buf = Vec::with_capacity(ser.buf_len);
        buf.resize(ser.buf_len,0);
        match sock.recv_from(&mut buf[..]).await
        {
            Ok((len,addr)) => {

                let id = CallHasher::get_hash(&addr, hash_builder.build_hasher());
                let has = {
                    let map = linker_map.lock().await;
                    if let Some(link) = map.get(&id){
                        link.send(buf[0..len].to_vec()).await;
                        true
                    }else { false }
                };
                if !has
                {
                    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(channel_buf);
                    {
                        let mut map = linker_map.lock().await;
                        map.insert(id, tx);
                    }
                    {
                        let linker_map_cp = linker_map.clone();
                        let clients = ser.clients.clone();
                        let lid = ser.logic_id.clone();
                        let conf = ser.config.clone();
                        let handler_cp = ser.handler.clone();
                        let parser_cp = ser.parser.clone();
                        let plugs_cp = ser.plug_mgr.clone();
                        let dead_plugs_cp:Arc<_> = ser.dead_plug_mgr.clone();
                        let sock_cp = sock.clone();
                        ser.runtime.spawn(run_in(
                        clients,lid,conf,handler_cp,parser_cp,plugs_cp,dead_plugs_cp,sock_cp,rx,
                        addr,id,linker_map_cp
                    ));
                    }
                    {
                        let mut map = linker_map.lock().await;
                        let tx = map.get(&id).unwrap();
                        tx.send(buf[0..len].to_vec()).await;
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