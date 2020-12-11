//! Simple composite-service TCP echo server.
//!
//! Using the following command:
//!
//! ```sh
//! nc 127.0.0.1 8080
//! ```
//!
//! Start typing. When you press enter the typed line will be echoed back. The server will log
//! the length of each line it echos and the total size of data sent when the connection is closed.
mod config_build;
mod ab_client;
mod handler;

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

fn del_client(cs:& mut Arc<Mutex<HashMap<usize,Box<AbClient>>>>,id:usize)
{
    let mut cs_ = cs.lock().unwrap();
    if cs_.contains_key(&id)
    {
        cs_.remove(&id).unwrap();
    }
}

fn set_client_st(cs:& mut Arc<Mutex<HashMap<usize,Box<AbClient>>>>,id:usize,st:State)
{
    let mut cs_ = cs.lock().unwrap();
    if let Some(c) = cs_.get_mut(&id)
    {
        c.state = st;
    }
}

fn get_client_write_buf(cs:& mut Arc<Mutex<HashMap<usize,Box<AbClient>>>>,id:usize)->Option<Vec<u8>>
{
    let mut cs_ = cs.lock().unwrap();
    if let Some(c) = cs_.get_mut(&id)
    {
        let res = c.write_buf.clone();
        c.write_buf = None;
        return res;
    }
    None
}

fn read_form_buf(reading:&mut bool,buf:&[u8],n:usize,data:&mut Vec<u8>,buf_rest:&mut [u8],buf_rest_len:&mut usize)->bool{
    let mut has_rest = false;
    let mut end_idx = 0usize;
    for i in 0..n{
        if !(*reading){
            if buf[i] == TOKEN_BEGIN{
                *reading = true;
                continue;
            }
        }else{
            if buf[i] == TOKEN_END{
                *reading = false;
                has_rest = true;
                end_idx = i;
                break;
            }else{
                data.push(buf[i]);
            }
        }
    }
    if has_rest && end_idx < n
    {
        let mut j = 0;
        for i in end_idx..n {
            buf_rest[j] = buf[i];
            j += 1;
        }
        *buf_rest_len = j;
    }

    has_rest && end_idx < n
}

fn handle_request<'a>(reading:&mut bool,data:&mut Vec<u8>,buf_rest:&mut [u8],buf_rest_len:usize,f:&'a dyn Fn(&mut Vec<u8>))
{
    if !(*reading) && !data.is_empty(){
        // handle
        f(data);
        data.clear();
        if buf_rest_len > 0{
            let mut rest = [0u8;1024];
            let mut rest_len = 0usize;
            read_form_buf(reading,&buf_rest,buf_rest_len,data,&mut rest,&mut rest_len);
            handle_request(reading,data,&mut rest,rest_len,f);
        }
    }
}


const TOKEN_BEGIN:u8 = 7u8;
const TOKEN_END:u8 = 9u8;

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

    let listener = TcpListener::bind(config.addr).await?;

    loop {
        let mut logic_id_cp = logic_id_.clone();
        let mut ab_clients_cp = ab_clients.clone();
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
            let mut data_len = 0usize;
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

                handle_request(&mut reading,&mut data,&mut buf_rest,buf_rest_len,& |d|{
                     dbg!(&d);
                     let respose = handler.handle(d);
                    let mut write_bit:usize = 0;
                    loop {
                        match socket.try_write(respose.as_slice()) {
                            Ok(n) => {
                                write_bit += n;
                                if write_bit == respose.len() {
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
