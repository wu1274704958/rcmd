use std::sync::{Arc, PoisonError, MutexGuard};
use std::sync::Mutex;
use std::collections::HashMap;
use crate::model::user;
use crate::handler::SubHandle;
use crate::ab_client::AbClient;
use std::collections::hash_map::RandomState;
use crate::ext_code::*;

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

impl SubHandle for GetUser
{
    type ABClient = AbClient;
    type Id = usize;

    fn handle(&self, data: &[u8], len: u32, ext: u32, clients: &Arc<Mutex<HashMap<Self::Id, Box<Self::ABClient>, RandomState>>>, id: Self::Id) -> Option<(Vec<u8>, u32)> where Self::Id: Copy {
        if ext != EXT_GET_USERS {return  None;}
        let login = if let Ok(u) = self.user_map.lock()
        {
            u.get(&id).is_some()
        }else{false};
        if !login{
            return Some((vec![],EXT_ERR_NOT_LOGIN));
        }
        let mut res = vec![];
        if let Ok(um) = self.user_map.lock()
        {
            um.iter().for_each(|(i,it)|{
                if *i != id
                {
                    let lid =  match self.login_map.lock()
                    {
                        Ok(v) => {
                            v.get(&it.acc).unwrap().clone()
                        }
                        Err(e) => {
                            0
                        }
                    };
                    let u = user::GetUser{name:it.name.clone(),lid};
                    res.push(serde_json::to_value(&u).unwrap());
                }
            });
        }
        Some((serde_json::Value::Array(res).to_string().into_bytes(),EXT_GET_USERS))
    }
}