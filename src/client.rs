use tokio::net::TcpSocket;
use std::net::{SocketAddr, SocketAddrV4, IpAddr, Ipv4Addr};
use tokio::io;
use tokio::prelude::*;
use std::ffi::{CString, CStr};
use async_std::net::Shutdown;

#[tokio::main]
async fn main() ->  io::Result<()>
{
    let sock = TcpSocket::new_v4().unwrap();
    let mut stream = sock.connect(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080)).await?;


    if let Ok(()) = stream.writable().await
    {
        let n = [7u8,97,98,9,67,9,6,7,56,45,90,87,7,9];
        println!("write .............");
        stream.write(&n).await?;
    }

    Ok(())
}