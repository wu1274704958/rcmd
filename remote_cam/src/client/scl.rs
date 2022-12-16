use super::super::model;
use super::comm;
use super::extc::*;
use super::handlers;
use crate::client_plugs::p2p_plugs::P2PPlug;
use crate::context::GLOB_CXT;
use async_trait::async_trait;
use rcmd_suit::agreement::DefParser;
use rcmd_suit::client_handler::{DefHandler, Handle, SubHandle};
use rcmd_suit::client_plug::client_plug::ClientPluCollect;
use rcmd_suit::clients::udp_client::UdpClient;
use rcmd_suit::tools::{self, platform_handle};
use rcmd_suit::utils::udp_sender::DefUdpSender;
use std::collections::VecDeque;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::SystemTime;
use std::{io, time::Duration};
use tokio::net::UdpSocket;
use tokio::sync::Mutex;

struct AutoLogin {
    acc: String,
    pwd: String,
    msg_queue: Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>,
    dur: Duration,
    last: Arc<Mutex<SystemTime>>,
}

impl AutoLogin {
    pub fn new(
        acc: String,
        pwd: String,
        msg_queue: Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>,
        dur: Duration,
    ) -> AutoLogin {
        AutoLogin {
            acc,
            pwd,
            msg_queue,
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
        send(&self.msg_queue, s.into_bytes(), EXT_LOGIN).await;
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
pub async fn run(cmd: &String, is_runing: Arc<Mutex<bool>>) -> io::Result<()> {
    let args: Vec<_> = cmd.split(" ").collect();
    let args = match tools::parse_c_args_ex(args) {
        Ok(a) => a,
        Err(e) => {
            dbg!(e);
            return Ok(());
        }
    };
    dbg!(&args);
    let msg_queue = Arc::new(Mutex::new(VecDeque::<(Vec<u8>, u32)>::new()));
    if args.acc.is_some() {
        let user = model::user::MinUser {
            acc: args.acc.as_ref().unwrap().clone(),
            pwd: args.pwd.as_ref().unwrap().clone(),
        };
        let s = serde_json::to_string(&user).unwrap();
        send(&msg_queue, s.into_bytes(), EXT_LOGIN).await;
    }

    let mut handler = DefHandler::new();
    let auto_login_handler = Arc::new(AutoLogin::new(
        args.acc.as_ref().unwrap().clone(),
        args.pwd.as_ref().unwrap().clone(),
        msg_queue.clone(),
        Duration::from_secs(2),
    ));

    {
        handler.add_handler(Arc::new(handlers::err::Err {}));
        if args.acc.is_some() {
            handler.add_handler(auto_login_handler.clone());
        }
    }
    let plugs = ClientPluCollect::<P2PPlug>::new();
    let handler_ptr = Arc::new(handler);
    let client_plug_ptr = Arc::new(plugs);

    loop {
        if {
            let is_run = is_runing.lock().await;
            !*is_run
        } {
            break;
        }
        let msg_queue = msg_queue.clone();
        let client = Arc::new(UdpClient::<_, _, DefUdpSender>::with_msg_queue_runing(
            (args.ip, args.bind_port),
            handler_ptr.clone(),
            DefParser::new(),
            msg_queue.clone(),
            is_runing.clone(),
        ));

        let msg_split_ignore: Option<&Vec<u32>> = Some(&comm::IGNORE_EXT);
        let sock = Arc::new(UdpSocket::bind(client.bind_addr).await?);
        platform_handle(sock.as_ref());
        let addr = SocketAddr::new(IpAddr::V4(args.ip), args.port);
        let sender = Arc::new(DefUdpSender::New(sock.clone(), addr));
        let run = client.run::<P2PPlug, _>(
            sock,
            addr,
            msg_split_ignore,
            msg_split_ignore,
            client_plug_ptr.clone(),
            sender,
            async {},
        );
        auto_login_handler.send_login().await;
        run.await;
    }
    Ok(())
}

async fn send(queue: &Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>, data: Vec<u8>, ext: u32) {
    let mut a = queue.lock().await;
    {
        a.push_back((data, ext));
    }
}
fn log(str: &str) {
    if let Ok(c) = GLOB_CXT.lock() {
        c.toast(str, 0).unwrap();
    }
}
