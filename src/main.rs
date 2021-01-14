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

use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;
use std::thread::{ThreadId, Thread};
use tokio::runtime;
use crate::config_build::{ConfigBuilder};
use crate::ab_client::{AbClient, State};
use std::sync::{Arc, Mutex};
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


//fn main(){}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let config = ConfigBuilder::new()
        .thread_count(8)
        .build();

    let config = tools::parse_args(config);

    let rt = runtime::Builder::new_multi_thread()
        .worker_threads(config.thread_count)
        .build()
        .unwrap();

    let mut logic_id_ = Arc::new(Mutex::new(0usize));
    let mut ab_clients = Arc::new(Mutex::new(HashMap::<usize,Box<AbClient>>::new()));
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

    let handler_cp:Arc<_> = handler.into();
    let parser_cp:Arc<_> = parser.into();
    let plugs_cp:Arc<_> = plugs.into();
    let dead_plugs_cp:Arc<_> = dead_plugs.into();

    let listener = TcpListener::bind(config.addr).await?;
    println!("Init Success!!!!");
    loop {
        let mut logic_id_cp = logic_id_.clone();
        let mut ab_clients_cp = ab_clients.clone();
        let handler_cp = handler_cp.clone();
        let parser_cp = parser_cp.clone();
        let plugs_cp = plugs_cp.clone();
        let dead_plugs_cp:Arc<_> = dead_plugs_cp.clone();
        let (mut socket, _) = listener.accept().await?;

        rt.spawn(async move {
            let mut logic_id = 0usize;
            {
                let mut a = logic_id_cp.lock().unwrap();
                a.add_assign(1usize);
                logic_id = *a;
            }
            let thread_id = std::thread::current().id();
            println!("thrend id = {:?}",thread_id);

            let local_addr = socket.local_addr().unwrap();
            let addr = socket.peer_addr().unwrap();


            {
                let mut ab_client = AbClient::new(local_addr, addr, logic_id, thread_id);
                let mut a = ab_clients_cp.lock().unwrap();
                a.insert(logic_id,Box::new(ab_client));
            }
            socket.readable().await;

            let mut buf = Vec::with_capacity(1024*1024*10);
            buf.resize(1024*1024*10,0);
            let mut subpackager = DefSubpackage::new();
            let mut asy = DefAsyCry::new();
            let mut spliter = DefMsgSplit::new();
            let mut package = None;
            // In a loop, read data from the socket and write the data back.
            loop {

                {
                    let st = get_client_st(&ab_clients_cp,logic_id);
                    if st.is_none() { return; }
                    match st{
                        Some(WaitKill) => {
                            dead_plugs_cp.run(logic_id,&mut ab_clients_cp,&config);
                            if del_client(&mut ab_clients_cp,logic_id) == 0{
                                if let Ok(mut l) = logic_id_cp.lock(){ *l = 0; }
                            }
                            return;
                        }
                        _ => {}
                    };
                }
                // read request
                //println!("{} read the request....",logic_id);
                match socket.try_read(&mut buf) {
                    Ok(0) => {
                        //println!("ok n == 0 ----");
                        dead_plugs_cp.run(logic_id,&mut ab_clients_cp,&config);
                        if del_client(&mut ab_clients_cp,logic_id) == 0{
                            if let Ok(mut l) = logic_id_cp.lock(){ *l = 0; }
                        }
                        return;
                    },
                    Ok(n) => {
                        //println!("n = {}",n);
                        set_client_st(&mut ab_clients_cp, logic_id, Busy);
                        let b = SystemTime::now();
                        package = subpackager.subpackage(&buf[0..n],n);
                        let e = SystemTime::now();
                        println!("subpackage use {} ms",e.duration_since(b).unwrap().as_millis());

                    }
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        //println!("e  WouldBlock -------");
                    }
                    Err(e) => {
                        eprintln!("error = {}", e);
                        dead_plugs_cp.run(logic_id,&mut ab_clients_cp,&config);
                        if del_client(&mut ab_clients_cp,logic_id) == 0{
                            if let Ok(mut l) = logic_id_cp.lock(){ *l = 0; }
                        }
                        return;
                    }
                };

                if package.is_none() && subpackager.need_check(){
                    let b = SystemTime::now();
                    package = subpackager.subpackage(&[],0);
                    println!("subpackage check use {} ms",SystemTime::now().duration_since(b).unwrap().as_millis());
                }

                if let Some(mut d) = package
                {
                    package = None;
                    let mut temp_data = None;
                    //handle request
                    let msg = {  parser_cp.parse_tf(&mut d) };
                    //dbg!(&msg);
                    if let Some(mut m) = msg {
                        //----------------------------------
                        let mut immediate_send = None;
                        let mut override_msg = None;
                        match asy.try_decrypt(m.msg,m.ext)
                        {
                            EncryptRes::EncryptSucc(d) => {
                                override_msg = Some(d);
                            }
                            EncryptRes::RPubKey(d) => {
                                immediate_send = Some(d.0);
                                m.ext = d.1;
                            }
                            EncryptRes::ErrMsg(d) => {
                                immediate_send = Some(d.0);
                                m.ext = d.1;
                            }
                            EncryptRes::NotChange => {}
                            EncryptRes::Break => {continue;}
                        };
                        if let Some(v) = immediate_send
                        {
                            let mut real_pkg = parser_cp.package_nor(v, m.ext);
                            socket.write(real_pkg.as_slice()).await;
                            continue;
                        }
                        if let Some(ref v) = override_msg
                        {
                            m.msg = v.as_slice();
                        }
                        let b = SystemTime::now();
                        if spliter.need_merge(&m)
                        {
                            if let Some((data,ext)) = spliter.merge(&m)
                            {
                                temp_data = Some(data);
                                m.ext = ext;
                                m.msg = temp_data.as_ref().unwrap().as_slice();
                            }else{
                                continue;
                            }
                        }
                        let respose = handler_cp.handle_ex(m, &ab_clients_cp, logic_id);
                        if m.ext != 9 {println!("handle ext {} use {} ms",m.ext,SystemTime::now().duration_since(b).unwrap().as_millis());}
                        if let Some((mut respose,mut ext)) = respose {
                            //---------------------------------
                            if spliter.need_split(respose.len(),ext)
                            {
                                let mut msgs = spliter.split(&mut respose,ext);
                                for i in msgs.into_iter(){
                                    let (mut data,ext,tag) = i;
                                    let mut send_data = match asy.encrypt(data, ext) {
                                        EncryptRes::EncryptSucc(d) => {
                                            d
                                        }
                                        _ => { data.to_vec()}
                                    };
                                    let mut real_pkg = parser_cp.package_tf(send_data, ext,tag);
                                    socket.write(real_pkg.as_slice()).await;
                                }
                            }else {
                                match asy.encrypt(&respose, ext) {
                                    EncryptRes::EncryptSucc(d) => {
                                        respose = d;
                                        println!("send ext {}", ext);
                                    }
                                    _ => {}
                                };
                                let mut real_pkg = parser_cp.package_nor(respose, ext);
                                socket.write(real_pkg.as_slice()).await;
                            }
                        }
                    }
                }

                //println!("{} handle the request....", logic_id);
                //println!("{} check the write_buf....", logic_id);
                let mut msg = None;
                {
                    if let Some(mut cl) = ab_clients_cp.lock().unwrap().get(&logic_id)
                    {
                        msg = cl.pop_msg();
                    }
                }
                        //------------------------------------------------
                if let Some((mut data,e)) = msg{
                    if spliter.need_split(data.len(),e)
                    {
                        let mut msgs = spliter.split(&mut data,e);
                        for i in msgs.into_iter(){
                            let (mut data,ext,tag) = i;
                            let mut send_data = match asy.encrypt(data, ext) {
                                EncryptRes::EncryptSucc(d) => {
                                    d
                                }
                                _ => { data.to_vec()}
                            };
                            let mut real_pkg = parser_cp.package_tf(send_data, ext, tag);
                            socket.write(real_pkg.as_slice()).await;
                        }
                    }else {
                        match asy.encrypt(&data, e) {
                            EncryptRes::EncryptSucc(d) => {
                                data = d;
                            }
                            _ => {}
                        };
                        let real_pkg = parser_cp.package_nor(data, e);
                        socket.write(real_pkg.as_slice()).await;
                    }
                }else {
                    async_std::task::sleep(config.min_sleep_dur).await;
                }

                set_client_st(&mut ab_clients_cp,logic_id,Ready);
                plugs_cp.run(logic_id,&mut ab_clients_cp,&config);
            }
        });
    }
}
