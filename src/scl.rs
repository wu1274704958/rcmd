use tokio::{ time::{ sleep}};
mod extc;
mod model;
mod client_handlers;
#[macro_use]
extern crate lazy_static;
mod comm;

use std::{sync::{Arc, Mutex}, time::Duration, io};
use std::time::SystemTime;
use std::collections::VecDeque;
use std::fs::{OpenOptions};
use std::io::*;
use extc::*;
use rcmd_suit::client_handler::{SubHandle, DefHandler, Handle};
use rcmd_suit::clients::tcp_client::TcpClient;
use rcmd_suit::agreement::DefParser;
use rcmd_suit::tools;


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
    fn handle(&self, _data: &[u8], _len: u32, ext: u32) -> Option<(Vec<u8>, u32)> {
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
#[tokio::main]
async fn main() -> io::Result<()>
{
    let cnf = OpenOptions::new().read(true).open("cnf");
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
    let msg_queue = Arc::new(Mutex::new(VecDeque::<(Vec<u8>, u32)>::new()));
    if args.acc.is_some(){
        let user = model::user::MinUser{ acc: args.acc.as_ref().unwrap().clone(),pwd:args.pwd.as_ref().unwrap().clone()};
        let s = serde_json::to_string(&user).unwrap();
        send(&msg_queue,s.into_bytes(),EXT_LOGIN);
    }
    
    let mut handler = DefHandler::new();

    {
        handler.add_handler(Arc::new(client_handlers::err::Err{}));
        handler.add_handler(Arc::new(client_handlers::exec_cmd::Exec::new()));
        handler.add_handler(Arc::new(client_handlers::save_file::SaveFile::with_observer(Box::new(on_save_file))));
        handler.add_handler(Arc::new(client_handlers::pull_file::PullFile::new(msg_queue.clone())));
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
        lazy_static::initialize(&comm::IGNORE_EXT);
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

fn send(queue: &Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>, data: Vec<u8>,ext:u32) {
    let mut a = queue.lock().unwrap();
    {
        a.push_back((data,ext));
    }
}
