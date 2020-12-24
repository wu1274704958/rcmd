use tokio::net::TcpSocket;
use std::net::{SocketAddr, SocketAddrV4, IpAddr, Ipv4Addr};
use tokio::{io, runtime};
use tokio::prelude::*;
use std::ffi::{CString, CStr};
use async_std::net::Shutdown;
use tokio::time::Duration;

mod config_build;
mod ab_client;
mod handler;
mod tools;
mod agreement;
mod asy_cry;
mod data_transform;

use tools::*;
use agreement::*;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use std::env;
use std::str::FromStr;

use asy_cry::*;
use data_transform::def_compress::DefCompress;
use std::env::consts::OS;
use std::collections::VecDeque;
use tokio::runtime::Runtime;

#[tokio::main]
async fn main() -> io::Result<()>
{
    let args = env::args();
    let mut ip = Ipv4Addr::new(127, 0, 0, 1);
    let mut port = 8080u16;
    if args.len() > 1
    {
        args.enumerate().for_each(|it|
            {
                if it.0 == 1
                {
                    if let Ok(i) = Ipv4Addr::from_str(it.1.as_str())
                    {
                        ip = i;
                    }
                }
                if it.0 == 2
                {
                    if let Ok(p) = u16::from_str(it.1.as_str())
                    {
                        port = p;
                    }
                }
            });
    }
    dbg!(ip);


    let mut msg_queue = Arc::new(Mutex::new(VecDeque::<(Vec<u8>, u32)>::new()));
    let mut is_runing = Arc::new(Mutex::new(true));

    let rt = runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .build()
        .unwrap();

    {
        let msg_queue = msg_queue.clone();
        let is_runing = is_runing.clone();
        rt.spawn(async move {
            loop {
                if let Ok(v) = is_runing.lock()
                {
                    if !*v {
                        break;
                    }
                } else {
                    break;
                }
                async_std::task::sleep(Duration::from_millis(10)).await;
            }
        });
    }

    {
        let msg_queue = msg_queue.clone();
        let is_runing = is_runing.clone();

        run(ip,port,msg_queue, is_runing).await?
    }
    Ok(())
}

async fn run(ip:Ipv4Addr,port:u16,mut msg_queue: Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>, is_runing: Arc<Mutex<bool>>) -> io::Result<()>
{
    let sock = TcpSocket::new_v4().unwrap();
    let mut stream = sock.connect(SocketAddr::new(IpAddr::V4(ip), port)).await?;
    let mut buf = [0u8; 1024];
    let mut reading = false;
    let mut data = Vec::new();
    let mut buf_rest = [0u8; 1024];
    let mut buf_rest_len = 0usize;
    // In a loop, read data from the socket and write the data back.
    let mut heartbeat_t = SystemTime::now();
    let mut pakager = DefParser::new();
    let mut asy = DefAsyCry::new();

    //pakager.add_transform(Arc::new(TestDataTransform{}));
    //pakager.add_transform(Arc::new(Test2DataTransform{}));

    //pakager.add_transform(Arc::new(DefCompress{}));

    let pub_key_data = asy.build_pub_key().unwrap();
    let real_pkg = real_package(pakager.package_tf(pub_key_data, 10));
    //dbg!(&real_pkg);
    stream.write(real_pkg.as_slice()).await;

    loop {
        /// read request
        //println!("{} read the request....",logic_id);
        match stream.try_read(&mut buf) {
            Ok(0) => {
                println!("ok n == 0 ----");
                break;
            }
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

        let mut requests = Vec::<Vec<u8>>::new();
        handle_request(&mut reading, &mut data, &mut buf_rest, buf_rest_len, &mut requests);
        for d in requests.iter_mut() {
            let msg = pakager.parse_tf(d);
            dbg!(&msg);
            if let Some(mut m) = msg {
                //----------------------------------
                let mut immediate_send = None;
                let mut override_msg = None;
                match asy.try_decrypt(m.msg, m.ext)
                {
                    EncryptRes::EncryptSucc(d) => {
                        override_msg = Some(d);
                    }
                    EncryptRes::RPubKey(d) => {
                        immediate_send = Some(d.0);
                        m.ext = d.1;
                    }
                    EncryptRes::ErrMsg((d)) => {
                        immediate_send = Some(d.0);
                        m.ext = d.1;
                    }
                    EncryptRes::NotChange => {}
                    EncryptRes::Break => { continue; }
                };
                if let Some(v) = immediate_send
                {
                    let mut real_pkg = real_package(pakager.package_tf(v, m.ext));
                    stream.write(real_pkg.as_slice()).await;
                    continue;
                }
                if let Some(ref v) = override_msg
                {
                    m.msg = v.as_slice();
                }

                dbg!(String::from_utf8_lossy(m.msg));
            }
        };

        if let Ok(n) = SystemTime::now().duration_since(heartbeat_t)
        {
            if n > Duration::from_secs_f32(17f32)
            {
                let pkg = real_package(pakager.package_tf(vec![9], 9));
                dbg!(&pkg);
                stream.write(pkg.as_slice()).await;
                heartbeat_t = SystemTime::now();
            }
        }

        if asy.can_encrypt() {
            let mut data = None;
            if let Ok(mut queue) = msg_queue.lock()
            {
                data = queue.pop_front();
            }
            if let Some(v) = data {
                match asy.encrypt(&v.0, v.1) {
                    EncryptRes::EncryptSucc(d) => {
                        let pkg = real_package(pakager.package_tf(d, 0));
                        stream.write(pkg.as_slice()).await;
                    }
                    _ => {}
                };
            }
        }

        if let Ok(v) = is_runing.lock()
        {
            if !*v {
                break;
            }
        } else {
            break;
        }
    }
    Ok(())
}