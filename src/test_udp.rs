#![allow(unused_imports)]
mod handlers;
mod extc;
mod model;
mod db;
#[macro_use]
extern crate rcmd_suit;
#[macro_use]
extern crate lazy_static;
mod comm;
mod p2p_handler;

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
use crate::db::db_mgr::DBMgr;
use rcmd_suit::utils::temp_permission::TempPermission;
use rcmd_suit::servers::udp_server::UdpServer;
use rcmd_suit::tools::platform_handle;
use rcmd_suit::servers::udp_server::run_udp_server;
use rcmd_suit::utils::udp_sender::{DefUdpSender, UdpSender};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>>{

    let config = ConfigBuilder::new()
        .thread_count(8)
        .build();

    let config = tools::parse_args(config);

    let mut handler = DefHandler::<TestHandler>::new();
    let mut parser = DefParser::new();
    let mut plugs = DefPlugMgr::<HeartBeat>::with_time();
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
    let msg_split_ignore:Option<&'static Vec<u32>> = Some(&comm::IGNORE_EXT);
    let asy_cry_ignore:Option<&'static Vec<u32>> = Some(&comm::IGNORE_EXT);
    //udp_server_run!(server,msg_split_ignore,msg_split_ignore);

    run_udp_server(server,msg_split_ignore,asy_cry_ignore).await;

    Ok(())
}

