use tokio::net::TcpSocket;
use std::net::{SocketAddr, SocketAddrV4, IpAddr, Ipv4Addr};
use tokio::{io, runtime};
use tokio::prelude::*;
use std::ffi::{CString, CStr};
use async_std::net::Shutdown;
use tokio::time::{Duration, sleep};

mod config_build;
mod ab_client;
mod handler;
mod utils;
mod agreement;
mod asy_cry;
mod data_transform;
mod ext_code;
mod subpackage;
mod model;
mod client_handlers;
mod tools;

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
use std::fs::{OpenOptions, File};
use std::io::*;
use ext_code::*;
use subpackage::{DefSubpackage,Subpackage};
use crate::client_handlers::def_handler::{ Handle,SubHandle};
use std::num::ParseIntError;
use args::ArgsError;
use utils::msg_split::{DefMsgSplit,MsgSplit};

struct AutoLogin{
    acc:String,
    pwd:String,
    msg_queue:Arc<Mutex<VecDeque<(Vec<u8>,u32)>>>,
    dur:Duration,
    last:Arc<Mutex<SystemTime>>
}

impl AutoLogin{
    pub fn new(acc:String,
               pwd:String,
               msg_queue:Arc<Mutex<VecDeque<(Vec<u8>,u32)>>>,
                dur:Duration)->AutoLogin
    {
        AutoLogin{acc,pwd,msg_queue,dur,last:Arc::new(Mutex::new(SystemTime::now()))}
    }
}

impl SubHandle for AutoLogin{
    fn handle(&self, data: &[u8], len: u32, ext: u32) -> Option<(Vec<u8>, u32)> {
        if ext == EXT_ERR_ALREADY_LOGIN {
            let mut last = self.last.lock().unwrap();
            if let Ok(dur) = SystemTime::now().duration_since(*last)
            {
                if dur < self.dur
                {
                    std::thread::sleep(self.dur - dur);
                }
                let user = model::user::MinUser { acc: self.acc.clone(), pwd: self.pwd.clone() };
                let s = serde_json::to_string(&user).unwrap();
                send(&self.msg_queue, s.into_bytes(), EXT_LOGIN);
                *last = SystemTime::now();
            }
        }
        None
    }
}

#[tokio::main]
async fn main() -> io::Result<()>
{
    let mut cnf = OpenOptions::new().read(true).open("cnf");
    let args =
        if let Ok(mut cnf) = cnf {
            let mut d = Vec::new();
            cnf.read_to_end(&mut d);
            let s = tools::decompress(&d);
            let args:Vec<_> = s.split(" ").collect();
            match tools::parse_c_args_ex(args)
            {
                Ok(a) => {a}
                Err(e) => {
                    dbg!(e);
                    return Ok(());
                }
            }
        }else{
            match tools::parse_c_args()
            {
                Ok(a) => {a}
                Err(e) => {
                    dbg!(e);
                    return Ok(());
                }
            }
        };

    dbg!(&args);
    let mut msg_queue = Arc::new(Mutex::new(VecDeque::<(Vec<u8>, u32)>::new()));
    if args.acc.is_some(){
        let user = model::user::MinUser{ acc: args.acc.as_ref().unwrap().clone(),pwd:args.pwd.as_ref().unwrap().clone()};
        let s = serde_json::to_string(&user).unwrap();
        send(&msg_queue,s.into_bytes(),EXT_LOGIN);
    }
    let mut is_runing = Arc::new(Mutex::new(true));
    let mut handler = client_handlers::def_handler::DefHandler::new();
    let mut msg_cache = Arc::new(Mutex::new(VecDeque::<(Vec<u8>,u32)>::new()));

    {
        handler.add_handler(Arc::new(client_handlers::err::Err{}));
        handler.add_handler(Arc::new(client_handlers::exec_cmd::Exec::new()));
        if args.acc.is_some(){
            handler.add_handler(Arc::new(AutoLogin::new(args.acc.as_ref().unwrap().clone(),
                                                        args.pwd.as_ref().unwrap().clone(),
            msg_queue.clone(),Duration::from_secs(2))));
        }
    }

    let handler = Arc::new(handler);

    let rt = runtime::Builder::new_multi_thread()
        .worker_threads(args.thread_num as usize)
        .build()
        .unwrap();

    for i in 0..args.thread_num{
        let msg_queue = msg_queue.clone();
        let is_runing = is_runing.clone();
        let msg_cache = msg_cache.clone();
        let handler = handler.clone();

        rt.spawn(async move{
            println!("on_handle...............");
            on_handle(msg_queue,is_runing,msg_cache,handler).await;
        });
    }

    loop{
        let msg_queue = msg_queue.clone();
        let is_runing = is_runing.clone();
        let handler = handler.clone();
        println!("runing.....");
        run(
            args.ip,
            args.port,
            msg_queue.clone(),
            is_runing,
            handler,
            msg_cache.clone()
        ).await;
        println!("next times...");
        if let Ok(mut mq) = msg_queue.lock(){
            mq.clear();
        }
        if let Ok(mut mq) = msg_cache.lock(){
            mq.clear();
        }
        if args.acc.is_some(){
            let user = model::user::MinUser{ acc: args.acc.as_ref().unwrap().clone(),pwd:args.pwd.as_ref().unwrap().clone()};
            let s = serde_json::to_string(&user).unwrap();
            send(&msg_queue,s.into_bytes(),EXT_LOGIN);
        }
        sleep(Duration::from_secs(1)).await;
    }
    Ok(())
}

async fn on_handle<T>(mut msg_queue: Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>,
                   is_runing: Arc<Mutex<bool>>,
                   msg_cache:Arc<Mutex<VecDeque<(Vec<u8>,u32)>>>,
                   handler:Arc<T>) -> io::Result<()>
where T:client_handlers::def_handler::Handle
{
    loop {
        if let Ok(v) = is_runing.lock()
        {
            if !*v {
                break;
            }
        } else {
            break;
        }
        let mut msg = None;
        {
            if let Ok(mut mc) = msg_cache.lock() {
                if let Some (m) = mc.pop_front(){
                    msg = Some(m);
                }
            }
        }
        match msg {
            None => {
                std::thread::sleep(Duration::from_millis(1));
            }
            Some(m) => {
                let msg_ = Message{
                    len : m.0.len() as u32,
                    msg : m.0.as_slice(),
                    ext : m.1,
                    tag : TOKEN_NORMAL
                };
                if let Some(d) = handler.handle_ex(msg_)
                {
                    send(&msg_queue,d.0,d.1);
                }
            }
        }
    }
    Ok(())
}

fn send(queue: &Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>, data: Vec<u8>,ext:u32) {
    let mut a = queue.lock().unwrap();
    {
        a.push_back((data,ext));
    }
}

async fn run(ip:Ipv4Addr,port:u16,mut msg_queue: Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>, is_runing: Arc<Mutex<bool>>,
             handler:Arc<client_handlers::def_handler::DefHandler>,
             msg_cache:Arc<Mutex<VecDeque<(Vec<u8>,u32)>>>) -> io::Result<()>
{
    let sock = TcpSocket::new_v4().unwrap();
    let mut stream = sock.connect(SocketAddr::new(IpAddr::V4(ip), port)).await?;
    let mut buf = [0u8; 1024];
    // In a loop, read data from the socket and write the data back.
    let mut heartbeat_t = SystemTime::now();
    let mut pakager = DefParser::new();
    let mut asy = DefAsyCry::new();
    let mut spliter = DefMsgSplit::new();
    let mut package = None;

    //pakager.add_transform(Arc::new(TestDataTransform{}));
    //pakager.add_transform(Arc::new(Test2DataTransform{}));

    //pakager.add_transform(Arc::new(DefCompress{}));

    if let Ok(pub_key_data) = asy.build_pub_key(){
        let real_pkg = pakager.package_nor(pub_key_data, 10);
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
            let mut temp_data = None;
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
                    let mut real_pkg = pakager.package_nor(v, m.ext);
                    stream.write(real_pkg.as_slice()).await;
                    continue;
                }
                if let Some(ref v) = override_msg
                {
                    m.msg = v.as_slice();
                }
                // if m.ext != 9
                // {println!("{:?} {}",&m.msg,m.ext);}
                if spliter.need_merge(&m)
                {
                    if let Some((data,ext)) = spliter.merge(&m)
                    {
                        temp_data = Some(data);
                        m.ext = ext;
                        m.msg = temp_data.as_ref().unwrap().as_slice();
                    }else{
                        continue;
                    }
                }
                {
                    if let Ok(mut mc) = msg_cache.lock() {
                        mc.push_back((m.msg.to_vec(),m.ext));
                    }
                }

                //dbg!(String::from_utf8_lossy(m.msg));
            }
            package = None;
        }

        if let Ok(n) = SystemTime::now().duration_since(heartbeat_t)
        {
            if n > Duration::from_secs_f32(10f32)
            {
                heartbeat_t = SystemTime::now();
                let pkg = pakager.package_nor(vec![9], 9);
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
            if let Some(mut v) = data {
                if spliter.need_split(v.0.len())
                {
                    let mut msgs = spliter.split(v.0,v.1);
                    for i in msgs.into_iter(){
                        let (mut data,ext,tag) = i;
                        match asy.encrypt(&data, ext) {
                            EncryptRes::EncryptSucc(d) => {
                                data = d;
                            }
                            _ => {}
                        };
                        let mut real_pkg = pakager.package_tf(data, ext,tag);
                        stream.write(real_pkg.as_slice()).await;
                    }
                }else {
                    match asy.encrypt(&v.0, v.1) {
                        EncryptRes::EncryptSucc(d) => {
                            v.0 = d;
                        }
                        EncryptRes::NotChange => {}
                        _ => {}
                    };
                    let pkg = pakager.package_nor(v.0, v.1);
                    stream.write(pkg.as_slice()).await;
                }
            }else{
                sleep(Duration::from_millis(1)).await;
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