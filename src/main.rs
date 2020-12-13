
mod config_build;
mod ab_client;
mod handler;
mod tools;
mod agreement;

use tokio::net::TcpListener;
use tokio::prelude::*;
use std::thread::{ThreadId, Thread};
use tokio::runtime;
use crate::config_build::{ConfigBuilder};
use crate::ab_client::{AbClient, State};
use std::sync::{Arc, Mutex};
use std::cell::RefCell;
use std::ops::AddAssign;
use crate::ab_client::State::{Dead, Busy, Ready};
use std::borrow::BorrowMut;
use std::collections::HashMap;
use tokio::time::Duration;
use crate::handler::{Handle, TestHandler};
use crate::tools::{read_form_buf, set_client_st, del_client, get_client_write_buf, handle_request, TOKEN_BEGIN, TOKEN_END, real_package};
use crate::agreement::{DefParser, Agreement,TestDataTransform,Test2DataTransform};


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let config = ConfigBuilder::new()
        .thread_count(4)
        .addr_s("127.0.0.1:8080")
        .build();

    let rt = runtime::Builder::new_multi_thread()
        .worker_threads(config.thread_count)
        .build()
        .unwrap();

    let mut logic_id_ = Arc::new(Mutex::new(0usize));
    let mut ab_clients = Arc::new(Mutex::new(HashMap::<usize,Box<AbClient>>::new()));
    let mut handler = TestHandler{};
    let mut parser = DefParser::new();
    parser.add_transform(Arc::new(TestDataTransform{}));
    parser.add_transform(Arc::new(Test2DataTransform{}));

    let listener = TcpListener::bind(config.addr).await?;

    loop {
        let mut logic_id_cp = logic_id_.clone();
        let mut ab_clients_cp = ab_clients.clone();
        let (mut socket, _) = listener.accept().await?;
        let parser_cp = parser.clone();
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
                let mut st = Ready;
                /// read request
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
                /// handle request
                //dbg!(&buf_rest);

                handle_request(&mut reading,&mut data,&mut buf_rest,buf_rest_len,&mut |d|{

                    let msg = parser_cp.parse_tf(d);
                    dbg!(&msg);
                    if let Some(ref m) = msg {
                        let respose = handler.handle(m, &mut ab_clients_cp, logic_id);
                        let mut pkg = parser_cp.package_tf(respose,0);
                        let mut real_pkg = real_package(pkg);
                        let mut write_bit: usize = 0;
                        let max_bit = real_pkg.len();
                        loop {
                            match socket.try_write(real_pkg.as_slice()) {
                                Ok(n) => {
                                    write_bit += n;
                                    if write_bit == max_bit {
                                        break;
                                    }
                                }
                                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                    continue;
                                }
                                Err(e) => {
                                    eprintln!("write error = {}", e);
                                    break;
                                }
                            }
                        }
                    }
                });
                //println!("{} handle the request....", logic_id);
                //println!("{} check the write_buf....", logic_id);

                if let Some(w_buf) = get_client_write_buf(&mut ab_clients_cp,logic_id)
                {
                    socket.write(w_buf.as_slice()).await;
                }else {
                    async_std::task::sleep(Duration::from_millis(10)).await;
                }
                set_client_st(&mut ab_clients_cp,logic_id,Ready);
            }
        });
    }
}
