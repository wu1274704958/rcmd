use std::sync::Arc;
use std::sync::Mutex;
use std::collections::HashMap;
use crate::model;
use crate::model::user;
use std::collections::hash_map::RandomState;
use crate::extc::*;
use terminal::Color;
use rcmd_suit::client_handler::SubHandle;

pub struct GetUser
{

}

impl GetUser {
    pub fn new()->GetUser
    {
        GetUser{

        }
    }
}

impl SubHandle for GetUser
{
    fn handle(&self, data: &[u8], len: u32, ext: u32) -> Option<(Vec<u8>, u32)> {

        let s = String::from_utf8_lossy(data).to_string();
        let u = serde_json::from_str::<serde_json::Value>(s.as_str()).unwrap();

        if let serde_json::Value::Array(v) = u{
            let out = terminal::stdout();
            out.batch(terminal::Action::SetForegroundColor(Color::Magenta));
            out.flush_batch();
            v.iter().for_each(|it|{
                let u = serde_json::from_value::<user::GetUser>(it.clone()).unwrap();
                println!("{} ---  {}",u.lid,u.name);
            });
            out.batch(terminal::Action::SetForegroundColor(Color::Reset));
            out.flush_batch();
        }

        None
    }

    fn interested(&self, ext:u32) ->bool {
        ext == EXT_GET_USERS
    }
}

pub struct RecvMsg
{

}

impl RecvMsg {
    pub fn new()->RecvMsg
    {
        RecvMsg{

        }
    }
}

impl SubHandle for RecvMsg
{
    fn handle(&self, data: &[u8], len: u32, ext: u32) -> Option<(Vec<u8>, u32)> {
        if ext != EXT_RECV_MSG {return  None;}
        let s = String::from_utf8_lossy(data).to_string();
        let u = serde_json::from_str::<model::RecvMsg>(s.as_str()).unwrap();
        let out = terminal::stdout();
        out.batch(terminal::Action::SetForegroundColor(Color::Cyan));
        out.flush_batch();
        println!("msg : {} from {}",u.msg,u.from_name);
        out.batch(terminal::Action::SetForegroundColor(Color::Reset));
        out.flush_batch();
        None
    }
}