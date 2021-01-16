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
use std::fs::OpenOptions;
use std::io::*;
use ext_code::*;
use subpackage::{DefSubpackage,Subpackage};
use crate::client_handlers::def_handler::Handle;
use std::num::ParseIntError;
use args::ArgsError;
use utils::msg_split::{DefMsgSplit,MsgSplit};

#[tokio::main]
async fn main() -> io::Result<()>
{
    let args = match tools::parse_c_args()
    {
        Ok(a) => {a}
        Err(e) => {
            dbg!(e);
            return Ok(());
        }
    };

    let mut msg_queue = Arc::new(Mutex::new(VecDeque::<(Vec<u8>, u32)>::new()));
    if args.acc.is_some(){
        let user = model::user::MinUser{ acc: args.acc.unwrap(),pwd:args.pwd.unwrap()};
        let s = serde_json::to_string(&user).unwrap();
        send(&msg_queue,s.into_bytes(),EXT_LOGIN);
    }
    let mut is_runing = Arc::new(Mutex::new(true));
    let mut handler = client_handlers::def_handler::DefHandler::new();

    {
        handler.add_handler(Arc::new(client_handlers::get_users::GetUser::new()));
        handler.add_handler(Arc::new(client_handlers::get_users::RecvMsg::new()));
        handler.add_handler(Arc::new(client_handlers::err::Err{}));
        handler.add_handler(Arc::new(client_handlers::exec_cmd::Exec::new()));
        handler.add_handler(Arc::new(client_handlers::run_cmd::RunCmd::new()));
        handler.add_handler(Arc::new(client_handlers::send_file::SendFile::new()));
        handler.add_handler(Arc::new(client_handlers::save_file::SaveFile::with_observer(Box::new(on_save_file))));
        handler.add_handler(Arc::new(client_handlers::pull_file_ret::PullFileRet::new()));
    }

    let rt = runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .build()
        .unwrap();

    {
        let msg_queue = msg_queue.clone();
        let is_runing = is_runing.clone();
        rt.spawn(async move{
            console(msg_queue,is_runing).await;
        });
    }

    {
        let msg_queue = msg_queue.clone();
        let is_runing = is_runing.clone();

        run(
            args.ip,
            args.port,
            msg_queue,
            is_runing,
            Arc::new(handler)
        ).await?
    }
    Ok(())
}

async fn console(mut msg_queue: Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>, is_runing: Arc<Mutex<bool>>) -> io::Result<()>
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

                        let mut buf = Vec::with_capacity(1024*100);
                        buf.resize(1024*100,0);
                        let mut is_first = true;
                        loop {
                            let mut d = head_v.clone();
                            match f.read(&mut buf[..]){
                                Ok(n) => {
                                    //println!("==== {} ====",n);
                                    if n <= 0
                                    {
                                        send(&msg_queue,d,EXT_UPLOAD_FILE_ELF);
                                        break;
                                    }else{
                                        d.reserve(n);
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
            "2" => {
                if cmds.len() < 3 {continue;}
                let acc = cmds[1].trim().to_string();
                let pwd = cmds[2].trim().to_string();
                let user = model::user::MinUser{acc,pwd};
                let s = serde_json::to_string(&user).unwrap();
                send(&msg_queue,s.into_bytes(),EXT_LOGIN);
            }
            "3" => {
                send(&msg_queue,vec![],EXT_LOGOUT);
            }
            "4" => {
                if cmds.len() < 4 {continue;}
                let acc = cmds[1].trim().to_string();
                let pwd = cmds[2].trim().to_string();
                let name = cmds[3].trim().to_string();
                let user = model::user::RegUser{acc,pwd,name};
                let s = serde_json::to_string(&user).unwrap();
                send(&msg_queue,s.into_bytes(),EXT_REGISTER);
            }
            "5" => {
                send(&msg_queue,vec![],EXT_GET_USERS);
            }
            "6" => {
                if cmds.len() < 3 {continue;}
                let lid = match usize::from_str(cmds[1]){
                    Ok(v) => {v}
                    Err(e) => { dbg!(e); continue;}
                };
                let msg = cmds[2].trim().to_string();
                let su = model::SendMsg{lid,msg};
                send(&msg_queue,serde_json::to_string(&su).unwrap().into_bytes(),EXT_SEND_MSG);
            }
            "7" => {
                if cmds.len() < 2 {continue;}
                let msg = cmds[1].trim().to_string();
                send(&msg_queue,msg.into_bytes(),EXT_SEND_BROADCAST);
            }
            "8" => {
                if cmds.len() < 2 {continue;}
                let lid = match usize::from_str(cmds[1].trim()){
                    Ok(v) => {v}
                    Err(e) => { dbg!(e); continue;}
                };
                loop {
                    let mut vec = vec![];
                    loop {
                        if let Ok(c) = in_.read_u8().await {
                            if c != b'\n'
                            {
                                vec.push(c);
                            }else { break; }
                        }
                    }
                    let s = String::from_utf8_lossy(vec.as_slice()).trim().to_string();
                    if s.as_bytes()[0] == b'#'
                    {
                        handle_sub_cmd(lid,s,msg_queue.clone());
                        continue;
                    }
                    match s.trim(){
                        "quit" =>{
                            break;
                        }
                        _=>{
                            let su = model::SendMsg { lid, msg: s.trim().to_string() };
                            send(&msg_queue, serde_json::to_string(&su).unwrap().into_bytes(), EXT_RUN_CMD);
                        }
                    }
                }
            }
            _ => {
                let help = r"
                    1 upload file
                    2 login [acc pwd]
                    3 logout
                    4 register [acc pwd name]
                    5 user list
                    6 send_msg [lid msg]
                    7 broadcast [msg]
                ";
                println!("{}",help);
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
             handler:Arc<client_handlers::def_handler::DefHandler>) -> io::Result<()>
{
    let sock = TcpSocket::new_v4().unwrap();
    let mut stream = sock.connect(SocketAddr::new(IpAddr::V4(ip), port)).await?;
    let mut buf = Vec::with_capacity(1024 * 1024 * 10);
    buf.resize(1024 * 1024 * 10,0);
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
        stream.write_all(real_pkg.as_slice()).await;
    }

    let mut subpackager = DefSubpackage::new();

    loop {
        /// read request
        //println!("read the request....");
        match stream.try_read(&mut buf[..]) {
            Ok(0) => {
                println!("ok n == 0 ----");
                break;
            }
            Ok(n) => {
                //println!("n = {}", n);
                package = subpackager.subpackage(&buf[0..n],n);
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
                    stream.write_all(real_pkg.as_slice()).await;
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
                if let Some(d) = handler.handle_ex(m)
                {
                    send(&msg_queue,d.0,d.1);
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
                stream.write_all(pkg.as_slice()).await;
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
                if spliter.need_split(v.0.len(),v.1)
                {
                    let mut msgs = spliter.split(&mut v.0,v.1);
                    for i in msgs.into_iter(){
                        let (mut data,ext,tag) = i;
                        let mut send_data = match asy.encrypt(data, ext) {
                            EncryptRes::EncryptSucc(d) => {
                                d
                            }
                            _ => { data.to_vec()}
                        };
                        let mut real_pkg = pakager.package_tf(send_data, ext,tag);
                        stream.write_all(real_pkg.as_slice()).await;
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
                    stream.write_all(pkg.as_slice()).await;
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

fn on_save_file(name:&str,len:usize,ext:u32)
{
    match ext {
        EXT_SAVE_FILE|
        EXT_SAVE_FILE_CREATE => {
            println!("recv file {} {} bytes!",name,len);
        }
        EXT_SAVE_FILE_ELF => {
            println!("recv file {} complete!",name);
        }
        _=>{}
    }


}

fn handle_sub_cmd(lid:usize,mut s:String,mut msg_queue: Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>)
{
    s.remove(0);
    let cmds:Vec<&str> = s.split(" ").collect();

    match cmds[0].trim() {
        "1" => {
            if cmds.len() < 3 {return;}
            match OpenOptions::new().read(true).open(cmds[1])
            {
                Ok(mut f) => {
                    let mut head_v = lid.to_be_bytes().to_vec();
                    head_v.push(TOKEN_BEGIN);
                    cmds[2].trim().as_bytes().iter().for_each(|it|{head_v.push(*it)});
                    head_v.push(TOKEN_END);

                    let mut buf = Vec::with_capacity(1024 * 100);
                    buf.resize(1024 * 100,0);
                    let mut is_first = true;
                    loop {
                        let mut d = head_v.clone();
                        match f.read(&mut buf[..]){
                            Ok(n) => {
                                //println!("==== {} ====",n);
                                if n <= 0
                                {
                                    send(&msg_queue,d,EXT_SEND_FILE_ELF);
                                    break;
                                }else{
                                    d.reserve(n);
                                    for i in 0..n { d.push(buf[i]);  }
                                    send(&msg_queue,d,if is_first {EXT_SEND_FILE_CREATE}else{EXT_SEND_FILE});
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
                    return;
                }
            }
        }
        "2" => {
            if cmds.len() < 2 {return;}
            let mut head_v = lid.to_be_bytes().to_vec();
            let mut pull_msg = model::PullFileMsg{
                far_end_path : cmds[1].trim().to_string(),
                near_end_path : if cmds.len() >= 3 {
                    Some(cmds[2].trim().to_string())
                } else{None}
            };
            let s = serde_json::to_string(&pull_msg).unwrap();
            head_v.extend_from_slice(s.as_bytes());
            send(&msg_queue,head_v,EXT_PULL_FILE_S);
        }
        _ => {}
    }
}