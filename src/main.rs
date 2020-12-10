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

use tokio::net::TcpListener;
use tokio::prelude::*;
use std::thread::{ThreadId, Thread};
use tokio::runtime;
use crate::config_build::{ConfigBuilder};
use crate::ab_client::AbClient;
use std::sync::{Arc, Mutex};
use std::cell::RefCell;
use std::ops::AddAssign;
use crate::ab_client::State::{Dead, Busy, Ready};
use std::borrow::BorrowMut;

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
    let mut ab_clients = Arc::new(Mutex::new(Vec::<Box<AbClient>>::new()));

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

            let mut idx = 0usize;
            {
                let mut ab_client = AbClient::new(local_addr, addr, logic_id, thread_id, socket);
                let mut a = ab_clients_cp.lock().unwrap();
                idx = a.len();
                a.push(Box::new(ab_client));
            }

            // In a loop, read data from the socket and write the data back.
            loop {
                let mut abs = ab_clients_cp.lock().unwrap();

                if let Some(ab) = abs.get_mut(idx)
                {
                    if ab.logic_id == logic_id
                    {
                        let mut socket = &mut ab.socket;
                        let n = match socket.read(&mut buf).await {
                            // socket closed
                            Ok(n) => {
                                //println!("n = {}",n);
                                if n <= 0 {
                                    //ab.state = Dead;
                                    println!("laddr = {} addr = {} maybe already closed connect!", local_addr, addr);
                                    return;
                                }
                                //ab.state = Busy;
                                let msg = String::from_utf8_lossy(&buf).to_string();
                                println!("msg = {} len = {}", msg, n);
                                n
                            }
                            Err(e) => {
                               // ab.state = Dead;
                                eprintln!("failed to read from socket; err = {:?}", e);
                                return;
                            }
                        };
                        //ab.state = Ready;
                    }
                }

            }
        });
    }
}
