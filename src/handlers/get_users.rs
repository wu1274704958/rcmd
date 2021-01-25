use std::sync::{Arc, PoisonError, MutexGuard};
use tokio::sync::Mutex;
use std::collections::HashMap;
use crate::model::user;
use crate::handler::SubHandle;
use crate::ab_client::AbClient;
use std::collections::hash_map::RandomState;
use crate::ext_code::*;
use async_trait::async_trait;

pub struct GetUser
{
    login_map:Arc<Mutex<HashMap<String,usize>>>,
    user_map:Arc<Mutex<HashMap<usize,user::User>>>
}

impl GetUser {
    pub fn new(user_map:Arc<Mutex<HashMap<usize,user::User>>>,
               login_map:Arc<Mutex<HashMap<String,usize>>>)->GetUser
    {
        GetUser{
            login_map,
            user_map
        }
    }
}

#[async_trait]
impl SubHandle for GetUser
{
    type ABClient = AbClient;
    type Id = usize;

    async fn handle(&self, data: &[u8], len: u32, ext: u32, clients: &Arc<Mutex<HashMap<Self::Id, Box<Self::ABClient>, RandomState>>>, id: Self::Id) -> Option<(Vec<u8>, u32)> where Self::Id: Copy {
        if ext != EXT_GET_USERS {return  None;}
        let login = {
            let u = self.user_map.lock().await;
            u.get(&id).is_some()
        };
        if !login{
            return Some((vec![],EXT_ERR_NOT_LOGIN));
        }
        let mut res = vec![];
        let um = self.user_map.lock().await;
        {
            let ns:Vec<_> = um.iter().map(|(i,it)|{
                it.acc.clone()
            }).collect();

            for i in ns{
                let v = self.login_map.lock().await;
                let lid = v.get(&i).unwrap().clone();
                let u = user::GetUser{name:i,lid};
                res.push(serde_json::to_value(&u).unwrap());
            }
        }
        Some((serde_json::Value::Array(res).to_string().into_bytes(),EXT_GET_USERS))
    }
}