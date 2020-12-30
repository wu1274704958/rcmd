#![allow(unused_imports)]
mod config_build;
mod ab_client;
mod handler;
mod tools;
mod agreement;
mod plug;
mod plugs;
mod handlers;
mod asy_cry;
mod data_transform;
mod ext_code;
mod subpackage;

use tokio::net::TcpListener;
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

    {
        handler.add_handler(Arc::new(handlers::heart_beat::HeartbeatHandler{}));
        handler.add_handler(Arc::new(TestHandler{}));
        handler.add_handler(Arc::new(handlers::upload_file::UploadHandler::new()));

        //parser.add_transform(Arc::new(DefCompress{}));

        plugs.add_plug(Arc::new(HeartBeat{}));
    }

    let handler_cp:Arc<_> = handler.into();
    let parser_cp:Arc<_> = parser.into();
    let plugs_cp:Arc<_> = plugs.into();

    let listener = TcpListener::bind(config.addr).await?;

    loop {
        let mut logic_id_cp = logic_id_.clone();
        let mut ab_clients_cp = ab_clients.clone();
        let handler_cp = handler_cp.clone();
        let parser_cp = parser_cp.clone();
        let plugs_cp = plugs_cp.clone();
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

            let mut buf = [0; 1024];
            let mut subpackager = DefSubpackage::new();
            let mut asy = DefAsyCry::new();
            let mut package = None;
            // In a loop, read data from the socket and write the data back.
            loop {

                {
                    let st = get_client_st(&ab_clients_cp,logic_id);
                    if st.is_none() { return; }
                    match st{
                        Some(WaitKill) => {
                            del_client(&mut ab_clients_cp,logic_id);
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
                        del_client(&mut ab_clients_cp,logic_id);
                        return;
                    },
                    Ok(n) => {
                        //println!("n = {}",n);
                        set_client_st(&mut ab_clients_cp, logic_id, Busy);
                        let b = SystemTime::now();
                        package = subpackager.subpackage(&buf,n);
                        let e = SystemTime::now();
                        println!("subpackage use {} ms",e.duration_since(b).unwrap().as_millis());

                    }
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        //println!("e  WouldBlock -------");
                    }
                    Err(e) => {
                        eprintln!("error = {}", e);
                        del_client(&mut ab_clients_cp,logic_id);
                        return;
                    }
                };

                if package.is_none() && subpackager.need_check(){
                    package = subpackager.subpackage(&[],0);
                }

                if let Some(mut d) = package
                {
                    package = None;
                    //handle request
                    let msg = parser_cp.parse_tf(&mut d);
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
                            let mut real_pkg = parser_cp.package_tf(v, m.ext);
                            socket.write(real_pkg.as_slice()).await;
                            continue;
                        }
                        if let Some(ref v) = override_msg
                        {
                            m.msg = v.as_slice();
                        }
                        let b = SystemTime::now();
                        let respose = handler_cp.handle_ex(m, &ab_clients_cp, logic_id);
                        println!("handle ext {} use {} ms",m.ext,SystemTime::now().duration_since(b).unwrap().as_millis());
                        if let Some((mut respose,mut ext)) = respose {
                            //---------------------------------
                            match asy.encrypt(&respose,ext) {
                                EncryptRes::EncryptSucc(d) => {
                                    respose = d;
                                    println!("send ext {}",ext);
                                }
                                _ => {}
                            };
                            let mut real_pkg = parser_cp.package_tf(respose, ext);
                            socket.write(real_pkg.as_slice()).await;
                        }
                    }
                }

                //println!("{} handle the request....", logic_id);
                //println!("{} check the write_buf....", logic_id);

                if let Some(mut w_buf) = get_client_write_buf(&mut ab_clients_cp,logic_id)
                {
                    //------------------------------------------------
                    match asy.encrypt(&w_buf.0,w_buf.1) {
                        EncryptRes::EncryptSucc(d) => {
                            w_buf.0 = d;
                        }
                        _ => {}
                    };
                    let real_pkg = parser_cp.package_tf(w_buf.0,w_buf.1);
                    socket.write(real_pkg.as_slice()).await;
                }else {
                    async_std::task::sleep(config.min_sleep_dur).await;
                }
                set_client_st(&mut ab_clients_cp,logic_id,Ready);
                plugs_cp.run(logic_id,&mut ab_clients_cp,&config);
            }
        });
    }
}
