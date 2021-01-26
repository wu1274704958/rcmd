use tokio::net::UdpSocket;
use std::net::SocketAddr;
use std::env::args;
use std::ops::AddAssign;

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
    sock.send("abc".to_string().as_bytes()).await;
    loop{}
    Ok(())
}