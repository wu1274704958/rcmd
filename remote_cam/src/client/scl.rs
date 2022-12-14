use std::{sync::{Arc, Mutex}, time::Duration, io};
use std::time::SystemTime;
use std::collections::VecDeque;
use std::fs::{OpenOptions};
use std::io::*;
use super::extc::*;
use super::super::model;
use super::handlers;
use super::comm;
use rcmd_suit::client_handler::{SubHandle, DefHandler, Handle};
use rcmd_suit::clients::tcp_client::TcpClient;
use rcmd_suit::agreement::DefParser;
use rcmd_suit::tools;
use tokio::{ time::{ sleep}};
use async_trait::async_trait;
use crate::GLOB_CXT;

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
#[async_trait]
impl SubHandle for AutoLogin{
    async fn handle(&self, _data: &[u8], _len: u32, ext: u32) -> Option<(Vec<u8>, u32)> {
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

#[allow(unused_must_use)]
async fn run(cmd:&String) -> io::Result<()>
{
    let args:Vec<_> = cmd.split(" ").collect();
    let args = match tools::parse_c_args_ex(args)
    {
        Ok(a) => {a}
        Err(e) => {
            dbg!(e);
            return Ok(());
        }
    };
    dbg!(&args);
    let msg_queue = Arc::new(Mutex::new(VecDeque::<(Vec<u8>, u32)>::new()));
    if args.acc.is_some(){
        let user = model::user::MinUser{ acc: args.acc.as_ref().unwrap().clone(),pwd:args.pwd.as_ref().unwrap().clone()};
        let s = serde_json::to_string(&user).unwrap();
        send(&msg_queue,s.into_bytes(),EXT_LOGIN);
    }
    
    let mut handler = DefHandler::new();

    {
        handler.add_handler(Arc::new(handlers::err::Err{}));
        if args.acc.is_some(){
            handler.add_handler(Arc::new(AutoLogin::new(args.acc.as_ref().unwrap().clone(),
                                                        args.pwd.as_ref().unwrap().clone(),
            msg_queue.clone(),Duration::from_secs(2))));
        }
    }
    
    let handler = Arc::new(handler);
    loop{
        println!("runing.....");
        let client = TcpClient::with_msg_queue(handler.clone(),
        DefParser::new(),
            msg_queue.clone()
        );
        let msg_split_ignore:Option<&Vec<u32>> = Some(&comm::IGNORE_EXT);
        client.run(args.ip, args.port,msg_split_ignore,msg_split_ignore).await;
        println!("next times...");
        if let Ok(mut mq) = msg_queue.lock(){
            mq.clear();
        }
        if args.acc.is_some(){
            let user = model::user::MinUser{ acc: args.acc.as_ref().unwrap().clone(),pwd:args.pwd.as_ref().unwrap().clone()};
            let s = serde_json::to_string(&user).unwrap();
            send(&msg_queue,s.into_bytes(),EXT_LOGIN);
        }
        sleep(Duration::from_secs(3)).await;
    }
    
}

fn send(queue: &Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>, data: Vec<u8>,ext:u32) {
    let mut a = queue.lock().unwrap();
    {
        a.push_back((data,ext));
    }
}
fn log(str:&str) {
    if let Ok(c) = GLOB_CXT.lock(){
        c.toast(str,0);
    }
}
