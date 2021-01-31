use std::sync::{Arc, PoisonError, MutexGuard};
use tokio::sync::Mutex;
use std::collections::HashMap;
use crate::model;
use crate::model::user;
use rcmd_suit::handler::SubHandle;
use rcmd_suit::ab_client::AbClient;
use std::collections::hash_map::RandomState;
use crate::extc::*;
use crate::model::user::User;
use rcmd_suit::tools::real_package;
use async_trait::async_trait;

pub struct SendMsg
{
    login_map:Arc<Mutex<HashMap<String,usize>>>,
    user_map:Arc<Mutex<HashMap<usize,user::User>>>
}

impl SendMsg {
    pub fn new(user_map:Arc<Mutex<HashMap<usize,user::User>>>,
               login_map:Arc<Mutex<HashMap<String,usize>>>)->SendMsg
    {
        SendMsg{
            login_map,
            user_map
        }
    }
}

#[async_trait]
impl SubHandle for SendMsg
{
    type ABClient = AbClient;
    type Id = usize;

    async fn handle(&self, data: &[u8], len: u32, ext: u32, clients: &Arc<Mutex<HashMap<Self::Id, Box<Self::ABClient>, RandomState>>>, id: Self::Id) -> Option<(Vec<u8>, u32)> where Self::Id: Copy {
        if ext != EXT_SEND_BROADCAST && ext != EXT_SEND_MSG { return None;}

        let mut self_name = None;
        {
            let lm = self.user_map.lock().await;
            match lm.get(&id){
                None => {return Some((vec![],EXT_ERR_NOT_LOGIN));}
                Some(u) => { self_name = Some(u.name.clone())}
            };
        }
        if ext == EXT_SEND_MSG {
            let s = String::from_utf8_lossy(data).to_string();
            if let Ok(mut mu) = serde_json::from_str::<model::SendMsg>(s.as_str()) {
                if mu.lid == id{
                    return Some((vec![], EXT_ERR_BAD_TARGET));
                }
                let mut cls = clients.lock().await;
                match cls.get_mut(&mu.lid){
                    None => {
                        return Some((vec![], EXT_ERR_NOT_FOUND_LID));
                    }
                    Some(cl) => {
                        let msg = model::RecvMsg{lid:id,msg:mu.msg,from_name:self_name.unwrap()};
                        let v = serde_json::to_string(&msg).unwrap().into_bytes();
                        cl.push_msg(v,EXT_RECV_MSG);
                        return Some((vec![], EXT_SEND_MSG));
                    }
                };
            } else {
                return Some((vec![], EXT_ERR_PARSE_ARGS));
            }
        }else if ext == EXT_SEND_BROADCAST{
            let s = String::from_utf8_lossy(data).to_string();
            let msg = model::RecvMsg{lid:id,msg:s,from_name:self_name.unwrap()};
            let v = serde_json::to_string(&msg).unwrap().into_bytes();

            {
                let mut cls = clients.lock().await;
                cls.iter_mut().for_each(|(it,cl)|{
                    if it != &id{
                        cl.push_msg(v.clone(),EXT_RECV_MSG);
                    }
                });
                return Some((vec![],EXT_SEND_BROADCAST));
            }

        }
        None
    }
}