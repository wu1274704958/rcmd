
mod context;
mod client;
mod model;
mod client_plugs;

use std::{sync::Arc, thread::sleep, time::Duration};

use client::scl::run;
use tokio::{runtime::Runtime, sync::Mutex};

fn main() {
    let runtime = Runtime::new().unwrap();
    let str = "-i 127.0.0.1  -b 8081 -p 8080".to_string();
    runtime.spawn(run(str,Arc::new(Mutex::new(true))));
    sleep(Duration::from_secs(1000*1000));
    println!("---");
}