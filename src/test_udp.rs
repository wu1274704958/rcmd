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
use crate::servers::udp_server::{UdpServer,run_in,platform_handle};

use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use ab_client::ABClient;
use std::hash::BuildHasher;
use ahash::RandomState;
use ahash::CallHasher;
use async_std::io::{Error, ErrorKind};
use std::env::consts::OS;

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

    let server = UdpServer::new(
        handler.into(),
        parser.into(),
        plugs.into(),
        dead_plugs.into(),
        config
    );

    udp_server_run!(server);

    Ok(())
}