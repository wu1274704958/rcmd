#![allow(unused_imports)]
mod config_build;
mod ab_client;
mod handler;
mod tools;
mod agreement;
mod plug;
mod plugs;
mod handlers;


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
use crate::tools::{read_form_buf, set_client_st, del_client, get_client_write_buf, handle_request, TOKEN_BEGIN, TOKEN_END, real_package, get_client_st};
use crate::agreement::{DefParser, Agreement,TestDataTransform,Test2DataTransform};
use crate::plug::{DefPlugMgr, PlugMgr};
use crate::plugs::heart_beat::HeartBeat;
use std::env;
use std::net::Ipv4Addr;
use std::str::FromStr;


//fn main(){}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let args = env::args();
    let mut ip = Ipv4Addr::new(127, 0, 0, 1);
    let mut port = 8080u16;
    if args.len() > 1
    {
        args.enumerate().for_each(|it|
            {
                if it.0 == 1
                {
                    if let Ok(i) = Ipv4Addr::from_str(it.1.as_str())
                    {
                        ip = i;
                    }
                }
                if it.0 == 2
                {
                    if let Ok(p) = u16::from_str(it.1.as_str())
                    {
                        port = p;
                    }
                }
            });
    }
    println!("{}:{}",ip,port);

    let config = ConfigBuilder::new()
        .thread_count(4)
        .addr_s(format!("{}:{}",ip,port).as_str())
        .build();

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
            let mut buf = [0; 1024];
            let local_addr = socket.local_addr().unwrap();
            let addr = socket.peer_addr().unwrap();


            {
                let mut ab_client = AbClient::new(local_addr, addr, logic_id, thread_id);
                let mut a = ab_clients_cp.lock().unwrap();
                a.insert(logic_id,Box::new(ab_client));
            }
            socket.readable().await;

            let mut reading = false;
            let mut data = Vec::new();
            let mut buf_rest = [0u8;1024];
            let mut buf_rest_len = 0usize;
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
                        println!("ok n == 0 ----");
                        del_client(&mut ab_clients_cp,logic_id);
                        return;
                    },
                    Ok(n) => {
                        println!("n = {}",n);
                        set_client_st(&mut ab_clients_cp, logic_id, Busy);
                        read_form_buf(&mut reading,&buf,n,&mut data,&mut buf_rest,&mut buf_rest_len);
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
                // handle request
                //dbg!(&buf_rest);
                let mut requests = Vec::<Vec<u8>>::new();
                handle_request(&mut reading,&mut data,&mut buf_rest,buf_rest_len,&mut requests);
                for d in requests.iter_mut(){
                    let msg = parser_cp.parse_tf(d);
                    dbg!(&msg);
                    if let Some(m) = msg {
                        let respose = handler_cp.handle_ex(m, &ab_clients_cp, logic_id);
                        if let Some((respose,ext)) = respose {
                            let mut pkg = parser_cp.package_tf(respose, ext);
                            let mut real_pkg = real_package(pkg);
                            socket.write(real_pkg.as_slice()).await;
                        }
                    }
                };
                //println!("{} handle the request....", logic_id);
                //println!("{} check the write_buf....", logic_id);

                if let Some(w_buf) = get_client_write_buf(&mut ab_clients_cp,logic_id)
                {
                    socket.write(w_buf.as_slice()).await;
                }else {
                    async_std::task::sleep(config.min_sleep_dur).await;
                }
                set_client_st(&mut ab_clients_cp,logic_id,Ready);
                plugs_cp.run(logic_id,&mut ab_clients_cp,&config);
            }
        });
    }
}
