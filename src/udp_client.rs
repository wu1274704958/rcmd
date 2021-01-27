#![allow(unused_imports)]
mod config_build;
mod ab_client;
mod handler;
mod utils;
mod agreement;
mod asy_cry;
mod data_transform;
mod ext_code;
mod subpackage;
mod db;
mod model;
mod tools;

use tokio::net::UdpSocket;
use std::net::SocketAddr;
use std::env::args;
use std::ops::AddAssign;
use crate::agreement::{DefParser,Agreement};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>>{
    let a:Vec<_> = args().collect();
    let mut addr = "0.0.0.0:".to_string();
    let mut port = "8081".to_string();
    dbg!(&a);
    if a.len() > 1 { port = a[1].clone(); }
    addr.add_assign(port.as_str());
    dbg!(&addr);
    let sock = UdpSocket::bind(addr.parse::<SocketAddr>().unwrap()).await?;
    sock.connect("127.0.0.1:8080".parse::<SocketAddr>().unwrap()).await?;

    let parser = DefParser::new();
    let data = parser.package_nor(vec![9],9);

    sock.send(data.as_slice()).await;
    loop{}
    Ok(())
}