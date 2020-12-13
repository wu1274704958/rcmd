use tokio::net::TcpSocket;
use std::net::{SocketAddr, SocketAddrV4, IpAddr, Ipv4Addr};
use tokio::io;
use tokio::prelude::*;
use std::ffi::{CString, CStr};
use async_std::net::Shutdown;
use tokio::time::Duration;

mod config_build;
mod ab_client;
mod handler;
mod tools;
mod agreement;
use tools::*;
use agreement::*;
use std::sync::Arc;

#[tokio::main]
async fn main() ->  io::Result<()>
{
    let sock = TcpSocket::new_v4().unwrap();
    let mut stream = sock.connect(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080)).await?;
    let mut buf=[0u8;1024];
    let mut reading = false;
    let mut data_len = 0usize;
    let mut data = Vec::new();
    let mut buf_rest = [0u8;1024];
    let mut buf_rest_len = 0usize;
    // In a loop, read data from the socket and write the data back.

    let mut pakager = DefParser::new();
    pakager.add_transform(Arc::new(TestDataTransform{}));
    pakager.add_transform(Arc::new(Test2DataTransform{}));

    let pkg = pakager.package_tf("gello".as_bytes().to_vec(),1);
    dbg!("gello".as_bytes());
    let real_pkg = real_package(pkg);
    stream.write(real_pkg.as_slice()).await;

    loop {
        /// read request
        //println!("{} read the request....",logic_id);
        match stream.try_read(&mut buf) {
            Ok(0) => {
                println!("ok n == 0 ----");
                break;
            },
            Ok(n) => {
                println!("n = {}", n);
                read_form_buf(&mut reading, &buf, n, &mut data, &mut buf_rest, &mut buf_rest_len);
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                async_std::task::sleep(Duration::from_millis(10)).await;
                //println!("e  WouldBlock -------");
            }
            Err(e) => {
                eprintln!("error = {}", e);
                break;
            }
        };
        /// handle request
        //dbg!(&buf_rest);

        handle_request(&mut reading, &mut data, &mut buf_rest, buf_rest_len, &mut |d| {
            dbg!(&d);
        });
    }

    Ok(())
}