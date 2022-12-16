
mod context;
mod client;
mod model;
mod client_plugs;

use client::scl::run;
use tokio::runtime::Runtime;

fn main() {
    let runtime = Runtime::new().unwrap();
    runtime.spawn(run());
    println!("---");
}