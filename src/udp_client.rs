use tokio::{io,sync::Mutex};
use tokio::prelude::*;
mod extc;
mod model;
mod client_handlers;
#[macro_use]
extern crate lazy_static;
extern crate get_if_addrs;
mod comm;

use std::sync::{Arc};
use std::str::FromStr;


use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::*;
use extc::*;
use rcmd_suit::tools;
use rcmd_suit::clients::udp_client::UdpClient;
use std::net::{SocketAddr, Ipv4Addr, SocketAddrV4, IpAddr};
use rcmd_suit::agreement::DefParser;
use rcmd_suit::client_handler::{DefHandler, Handle};
use rcmd_suit::tools::{TOKEN_BEGIN, TOKEN_END, SEND_BUF_SIZE};
use rcmd_suit::utils::udp_sender::DefUdpSender;

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
    let ip = IpAddr::from_str("0.0.0.0").unwrap();

    let msg_queue = Arc::new(Mutex::new(VecDeque::<(Vec<u8>, u32)>::new()));
    if args.acc.is_some(){
        let user = model::user::MinUser{ acc: args.acc.unwrap(),pwd:args.pwd.unwrap()};
        let s = serde_json::to_string(&user).unwrap();
        send(&msg_queue,s.into_bytes(),EXT_LOGIN).await;
    }
    let is_runing = Arc::new(Mutex::new(true));
    let mut handler = DefHandler::new();

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

    let console = {
        let msg_queue = msg_queue.clone();
        console(msg_queue,is_runing.clone())
    };

    {
        let msg_queue = msg_queue.clone();
        let client = Arc::new( UdpClient::with_msg_queue_runing(
            (ip, args.bind_port),
            Arc::new(handler),
            DefParser::new(),
            msg_queue.clone(),
            is_runing
        ));
        lazy_static::initialize(&comm::IGNORE_EXT);
        let msg_split_ignore:Option<&Vec<u32>> = Some(&comm::IGNORE_EXT);
        let run = client.run::<DefUdpSender>(args.ip,args.port,msg_split_ignore,msg_split_ignore);
        futures::join!(console,run);
    }

    Ok(())
}
#[allow(unused_assignments)]
async fn console(msg_queue: Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>, is_runing: Arc<Mutex<bool>>) -> io::Result<()>
{
    loop {
        {
            let is_runing = is_runing.lock().await;
            if !*is_runing { break; }
        }
        let mut cmd = String::new();
        let mut vec = vec![];
        let mut in_ = tokio::io::stdin();
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
            "-" => {
                let mut v = is_runing.lock().await;
                *v = false;
                return Ok(());
            }
            "0" => {
                if cmds.len() < 2 {continue;}
                send(&msg_queue,cmds[1].into(),0).await;
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

                        let mut buf = Vec::with_capacity(SEND_BUF_SIZE);
                        buf.resize(SEND_BUF_SIZE,0);
                        let mut is_first = true;
                        loop {
                            let mut d = head_v.clone();
                            match f.read(&mut buf[..]){
                                Ok(n) => {
                                    //println!("==== {} ====",n);
                                    if n <= 0
                                    {
                                        send(&msg_queue,d,EXT_UPLOAD_FILE_ELF).await;
                                        break;
                                    }else{
                                        d.reserve(n);
                                        for i in 0..n { d.push(buf[i]);  }
                                        send(&msg_queue,d,if is_first {EXT_UPLOAD_FILE_CREATE}else{EXT_UPLOAD_FILE}).await;
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
                send(&msg_queue,s.into_bytes(),EXT_LOGIN).await;
            }
            "3" => {
                send(&msg_queue,vec![],EXT_LOGOUT).await;
            }
            "4" => {
                if cmds.len() < 4 {continue;}
                let acc = cmds[1].trim().to_string();
                let pwd = cmds[2].trim().to_string();
                let name = cmds[3].trim().to_string();
                let user = model::user::RegUser{acc,pwd,name};
                let s = serde_json::to_string(&user).unwrap();
                send(&msg_queue,s.into_bytes(),EXT_REGISTER).await;
            }
            "5" => {
                send(&msg_queue,vec![],EXT_GET_USERS).await;
            }
            "6" => {
                if cmds.len() < 3 {continue;}
                let lid = match usize::from_str(cmds[1]){
                    Ok(v) => {v}
                    Err(e) => { dbg!(e); continue;}
                };
                let msg = cmds[2].trim().to_string();
                let su = model::SendMsg{lid,msg};
                send(&msg_queue,serde_json::to_string(&su).unwrap().into_bytes(),EXT_SEND_MSG).await;
            }
            "7" => {
                if cmds.len() < 2 {continue;}
                let msg = cmds[1].trim().to_string();
                send(&msg_queue,msg.into_bytes(),EXT_SEND_BROADCAST).await;
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
                        handle_sub_cmd(lid,s,msg_queue.clone()).await;
                        continue;
                    }
                    match s.trim(){
                        "quit" =>{
                            break;
                        }
                        _=>{
                            let su = model::SendMsg { lid, msg: s.trim().to_string() };
                            send(&msg_queue, serde_json::to_string(&su).unwrap().into_bytes(), EXT_RUN_CMD).await;
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

async fn send(queue: &Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>, data: Vec<u8>,ext:u32) {
    let mut a = queue.lock().await;
    {
        a.push_back((data,ext));
    }
}

fn on_save_file(name:&str,len:usize,ext:u32)
{
    match ext {
        EXT_SAVE_FILE|
        EXT_SAVE_FILE_CREATE => {
            println!("recv file {} {} bytes!",name,len);
        }
        EXT_SAVE_FILE_ELF => {
            println!("recv file {} complete size {} bytes!",name,len);
        }
        _=>{}
    }


}

async fn handle_sub_cmd(lid:usize,mut s:String,msg_queue: Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>)
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

                    let mut buf = Vec::with_capacity(SEND_BUF_SIZE);
                    buf.resize(SEND_BUF_SIZE,0);
                    let mut is_first = true;
                    let mut bytes = 0;
                    loop {
                        let mut d = head_v.clone();
                        match f.read(&mut buf[..]){
                            Ok(n) => {
                                //println!("==== {} ====",n);
                                if n <= 0
                                {
                                    send(&msg_queue,d,EXT_SEND_FILE_ELF).await;
                                    println!("file size {}",bytes);
                                    break;
                                }else{
                                    d.reserve(n);
                                    d.extend_from_slice(&buf[0..n]);
                                    bytes += n;
                                    send(&msg_queue,d,if is_first {EXT_SEND_FILE_CREATE}else{EXT_SEND_FILE}).await;
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
            let pull_msg = model::PullFileMsg{
                far_end_path : cmds[1].trim().to_string(),
                near_end_path : if cmds.len() >= 3 {
                    Some(cmds[2].trim().to_string())
                } else{None}
            };
            let s = serde_json::to_string(&pull_msg).unwrap();
            head_v.extend_from_slice(s.as_bytes());
            send(&msg_queue,head_v,EXT_PULL_FILE_S).await;
        }
        _ => {}
    }
}