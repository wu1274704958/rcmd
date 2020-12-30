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
mod ext_code;
mod subpackage;

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
use std::fs::OpenOptions;
use std::io::*;
use ext_code::*;
use subpackage::{DefSubpackage,Subpackage};

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
        .worker_threads(2)
        .build()
        .unwrap();

    {
        let msg_queue = msg_queue.clone();
        let is_runing = is_runing.clone();
        rt.spawn(async move{
            loop {
                if let Ok(v) = is_runing.lock()
                {
                    if !*v {
                        break;
                    }
                } else {
                    break;
                }
                let mut cmd = String::new();
                let mut vec = vec![];
                let mut in_ = tokio::io::stdin();
                let mut c = b'\0';
                loop {
                    if let Ok(c) = in_.read_u8().await {
                        if c != b'\n'
                        {
                            vec.push(c);
                        }else { break; }
                    }
                }

                cmd = String::from_utf8_lossy(vec.as_slice()).to_string();
                println!(">>>{}",cmd);
                let cmds:Vec<&str> = cmd.split(" ").collect();

                match cmds[0] {
                    "0" => {
                        if cmds.len() < 2 {continue;}
                        send(&msg_queue,cmds[1].into(),0);
                    },
                    "1" => {
                        if cmds.len() < 3 {continue;}
                        match OpenOptions::new().read(true).open(cmds[1])
                        {
                            Ok(mut f) => {
                                let mut head_v = vec![];
                                head_v.push(TOKEN_BEGIN);
                                cmds[2].trim().as_bytes().iter().for_each(|it|{head_v.push(*it)});
                                head_v.push(TOKEN_END);

                                let mut buf = [0u8;490];
                                let mut is_first = true;
                                loop {
                                    let mut d = head_v.clone();
                                    match f.read(&mut buf){
                                        Ok(n) => {
                                            //println!("==== {} ====",n);
                                            if n <= 0
                                            {
                                                send(&msg_queue,d,EXT_UPLOAD_FILE_ELF);
                                                break;
                                            }else{
                                                for i in 0..n { d.push(buf[i]);  }
                                                send(&msg_queue,d,if is_first {EXT_UPLOAD_FILE_CREATE}else{EXT_UPLOAD_FILE});
                                                is_first = false;
                                            }
                                        }
                                        _=>{
                                        }
                                    }
                                }
                                //println!("==== end ====");
                            }
                            Err(e) => {
                                eprintln!("{}",e);
                            }
                        }
                    }
                    _ => {}
                }

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

fn send(queue: &Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>, data: Vec<u8>,ext:u32) {
    let mut a = queue.lock().unwrap();
    {
        a.push_back((data,ext));
    }
}

async fn run(ip:Ipv4Addr,port:u16,mut msg_queue: Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>, is_runing: Arc<Mutex<bool>>) -> io::Result<()>
{
    let sock = TcpSocket::new_v4().unwrap();
    let mut stream = sock.connect(SocketAddr::new(IpAddr::V4(ip), port)).await?;
    let mut buf = [0u8; 1024];
    // In a loop, read data from the socket and write the data back.
    let mut heartbeat_t = SystemTime::now();
    let mut pakager = DefParser::new();
    let mut asy = DefAsyCry::new();
    let mut package = None;

    //pakager.add_transform(Arc::new(TestDataTransform{}));
    //pakager.add_transform(Arc::new(Test2DataTransform{}));

    //pakager.add_transform(Arc::new(DefCompress{}));

    if let Ok(pub_key_data) = asy.build_pub_key(){
        let real_pkg = pakager.package_tf(pub_key_data, 10);
        //dbg!(&real_pkg);
        stream.write(real_pkg.as_slice()).await;
    }

    let mut subpackager = DefSubpackage::new();

    loop {
        /// read request
        //println!("read the request....");
        match stream.try_read(&mut buf) {
            Ok(0) => {
                println!("ok n == 0 ----");
                break;
            }
            Ok(n) => {
                //println!("n = {}", n);
                package = subpackager.subpackage(&buf,n);
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                //async_std::task::sleep(Duration::from_millis(10)).await;
                //println!("e  WouldBlock -------");
            }
            Err(e) => {
                eprintln!("error = {}", e);
                break;
            }
        };
        /// handle request
        //dbg!(&buf_rest);
        if package.is_none() && subpackager.need_check(){
            package = subpackager.subpackage(&[],0);
        }

        if let Some( mut d) = package {
            package = None;
            let msg = pakager.parse_tf(&mut d);
            //dbg!(&msg);
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
                    let mut real_pkg = pakager.package_tf(v, m.ext);
                    stream.write(real_pkg.as_slice()).await;
                    continue;
                }
                if let Some(ref v) = override_msg
                {
                    m.msg = v.as_slice();
                }
                if m.ext != 9
                {println!("{:?} {}",&m.msg,m.ext);}
                //dbg!(String::from_utf8_lossy(m.msg));
            }
            package = None;
        }

        if let Ok(n) = SystemTime::now().duration_since(heartbeat_t)
        {
            if n > Duration::from_secs_f32(17f32)
            {
                heartbeat_t = SystemTime::now();
                let pkg = pakager.package_tf(vec![9], 9);
                //dbg!(&pkg);
                stream.write(pkg.as_slice()).await;
                //println!("send heart beat");
            }
        }

        if asy.can_encrypt() {
            let mut data = None;
            {
                let mut queue = msg_queue.lock().unwrap();
                data = queue.pop_front();
            }
            if let Some(v) = data {

                match asy.encrypt(&v.0, v.1) {
                    EncryptRes::EncryptSucc(d) => {
                        //println!("{:?} ext: {}",&v.0, v.1);
                        let pkg = pakager.package_tf(d, v.1);
                        stream.write(pkg.as_slice()).await;
                    }
                    EncryptRes::NotChange => {
                        //println!("{:?} ext: {}",&v.0, v.1);
                        let pkg = pakager.package_tf(v.0, v.1);
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