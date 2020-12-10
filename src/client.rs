use tokio::net::TcpSocket;
use std::net::{SocketAddr, SocketAddrV4, IpAddr, Ipv4Addr};
use tokio::io;
use tokio::prelude::*;
use std::ffi::{CString, CStr};

#[tokio::main]
async fn main() ->  io::Result<()>
{
    let sock = TcpSocket::new_v4().unwrap();
    let mut stream = sock.connect(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080)).await?;


    if let Ok(()) = stream.writable().await
    {
        stream.try_write("hello".as_bytes());
    }

    if let Ok(()) = stream.readable().await
    {
        let mut buf = [0u8;100];
        let n = stream.read(&mut buf).await?;
        dbg!(buf);
        let msg = unsafe {CStr::from_bytes_with_nul_unchecked(&buf)};
        println!("msg = {:?} len = {}",msg,n);
    }

    Ok(())
}