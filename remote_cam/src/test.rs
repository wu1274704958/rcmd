
mod context;
mod client;
mod model;
mod client_plugs;

use std::{sync::Arc, thread::sleep, time::Duration};

use client::scl::run;
use tokio::{runtime::Runtime, sync::Mutex};
use crate::client::scl::NetContext;

fn main() {
    let runtime = Runtime::new().unwrap();
    let cxt = Arc::new(Mutex::new(NetContext::new()));
    let str = "-i 127.0.0.1  -b 8081 -p 8080 -a wws -s 31726".to_string();
    let res = runtime.spawn(run(str,cxt));
    while !res.is_finished() {
        sleep(Duration::from_secs(1));
    }
    println!("---");
}