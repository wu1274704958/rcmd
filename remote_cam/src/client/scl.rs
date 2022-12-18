use super::super::model;
use super::comm;
use super::extc::*;
use super::handlers;
use crate::client_plugs::p2p_plugs::P2PPlug;
#[cfg(target_os="android")]
use crate::context::GLOB_CXT;
use async_trait::async_trait;
use rcmd_suit::agreement::DefParser;
use rcmd_suit::client_handler::{DefHandler, Handle, SubHandle};
use rcmd_suit::client_plug::client_plug::ClientPluCollect;
use rcmd_suit::clients::udp_client::UdpClient;
use rcmd_suit::tools::{self, platform_handle};
use rcmd_suit::utils::udp_sender::DefUdpSender;
use tokio::time::sleep;
use std::collections::VecDeque;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::SystemTime;
use std::{io, time::Duration};
use tokio::net::UdpSocket;
use tokio::sync::Mutex;

pub struct NetContext{
    pub is_runing : Arc<Mutex<bool>>,
    pub loop_login : Arc<Mutex<bool>>,
    pub msg_queue : Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>
}

impl NetContext {
    pub fn new()-> NetContext
    {
        NetContext { 
            is_runing: Arc::new(Mutex::new(true)),
            loop_login: Arc::new(Mutex::new(true)),
            msg_queue: Arc::new(Mutex::new(VecDeque::<(Vec<u8>, u32)>::new()))
        }
    }
    pub async fn send(&self, data: Vec<u8>, ext: u32) {
        let mut a = self.msg_queue.lock().await;
        {
            a.push_back((data, ext));
        }
    }
    pub async fn clear_queue(&self) {
        let mut a = self.msg_queue.lock().await;
        {
            a.clear();
        }
    }
}

struct AutoLogin {
    acc: String,
    pwd: String,
    cxt: Arc<Mutex<NetContext>>,
    dur: Duration,
    last: Arc<Mutex<SystemTime>>,
}

impl AutoLogin {
    pub fn new(
        acc: String,
        pwd: String,
        cxt: Arc<Mutex<NetContext>>,
        dur: Duration,
    ) -> AutoLogin {
        AutoLogin {
            acc,
            pwd,
            cxt,
            dur,
            last: Arc::new(Mutex::new(SystemTime::now())),
        }
    }
    pub async fn send_login(&self) {
        let user = model::user::MinUser {
            acc: self.acc.clone(),
            pwd: self.pwd.clone(),
        };
        let s = serde_json::to_string(&user).unwrap();
        let cxt = self.cxt.lock().await;
        cxt.send(s.into_bytes(), EXT_LOGIN).await;
    }
}
#[async_trait]
impl SubHandle for AutoLogin {
    async fn handle(&self, _data: &[u8], _len: u32, ext: u32) -> Option<(Vec<u8>, u32)> {
        if ext == EXT_LOGIN {
            log("登录成功！！！");
        }
        None
    }
}

#[allow(unused_must_use)]
pub async fn run(cmd: String, cxt:Arc<Mutex<NetContext>>) -> io::Result<()> {
    log("into run");
    let args: Vec<_> = cmd.split(" ").collect();
    let args = match tools::parse_c_args_ex(args) {
        Ok(a) => a,
        Err(e) => {
            dbg!(e);
            return Ok(());
        }
    };
    dbg!(&args);
    let msg_queue = {
        let cxt = cxt.lock().await;
        cxt.msg_queue.clone()
    };
    

    let mut handler = DefHandler::new();
    let auto_login_handler = Arc::new(AutoLogin::new(
        args.acc.as_ref().unwrap().clone(),
        args.pwd.as_ref().unwrap().clone(),
        cxt.clone(),
        Duration::from_secs(2),
    ));
    if args.acc.is_some() {
        auto_login_handler.send_login().await;
    }
    {
        handler.add_handler(Arc::new(handlers::err::Err {}));
        if args.acc.is_some() {
            handler.add_handler(auto_login_handler.clone());
        }
    }
    let plugs = ClientPluCollect::<P2PPlug>::new();
    let handler_ptr = Arc::new(handler);
    let client_plug_ptr = Arc::new(plugs);

    let (is_runing,loop_login) = {
        let tmp = cxt.lock().await;
        (tmp.is_runing.clone(),tmp.loop_login.clone())
    };

    let msg_split_ignore: Option<&Vec<u32>> = Some(&comm::IGNORE_EXT);
    let sock = Arc::new(UdpSocket::bind((IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)), args.bind_port)).await.unwrap());
    platform_handle(sock.as_ref());
    let addr = SocketAddr::new(IpAddr::V4(args.ip), args.port);

    loop {
        log("launch client");
        let msg_queue = msg_queue.clone();
        let client = Arc::new(UdpClient::<_, _, DefUdpSender>::with_msg_queue_runing(
            (args.ip, args.bind_port),
            handler_ptr.clone(),
            DefParser::new(),
            msg_queue.clone(),
            is_runing.clone(),
        ));
        let sender = Arc::new(DefUdpSender::New(sock.clone(), addr));
        let run = client.run::<P2PPlug, _>(
            sock.clone(),
            addr,
            msg_split_ignore,
            msg_split_ignore,
            client_plug_ptr.clone(),
            sender,
            async {},
        );
        
        let res = run.await;
        let str = format!("run res = {:?}",res);
        log(str.as_str());
        if {
            let tmp = loop_login.lock().await;
            *tmp
        } {
            let mut tmp = is_runing.lock().await;
            *tmp = true;
            sleep(Duration::from_secs(1)).await;
            {
                let cxt = cxt.lock().await;
                cxt.clear_queue().await;
            }
            auto_login_handler.send_login().await;
            log("retry next times");
        }else{
            break;
        }
    }
    Ok(())
}


#[cfg(target_os="android")]
fn log(str: &str) {
    if let Ok(c) = GLOB_CXT.lock() {
        c.toast(str, 0).unwrap();
    }
}
#[cfg(target_os="windows")]
fn log(str: &str) {
    println!("{}",str);
}
