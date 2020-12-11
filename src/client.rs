use tokio::net::TcpSocket;
use std::net::{SocketAddr, SocketAddrV4, IpAddr, Ipv4Addr};
use tokio::io;
use tokio::prelude::*;
use std::ffi::{CString, CStr};
use async_std::net::Shutdown;
use tokio::time::Duration;


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

    stream.write(&[7,67,89,0,9,12,3,7,67,89,0,9]).await;

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

        handle_request(&mut reading, &mut data, &mut buf_rest, buf_rest_len, &|d| {
            dbg!(&d);
        });
    }

    Ok(())
}