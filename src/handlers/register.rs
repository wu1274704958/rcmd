use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashMap;
use crate::model::user;
use crate::handler::SubHandle;
use crate::ab_client::AbClient;
use std::collections::hash_map::RandomState;
use crate::ext_code::*;
use crate::model::user::{RegUser, User};
use regex::Regex;
use crate::db::db_mgr::DBMgr;
use mysql::{Error, PooledConn};
use mysql::prelude::Queryable;
use async_trait::async_trait;
pub struct Register
{
    db_mgr:Arc<DBMgr>,
    user_map:Arc<Mutex<HashMap<usize,user::User>>>
}

impl Register {
    pub fn new( db_mgr:Arc<DBMgr>,user_map:Arc<Mutex<HashMap<usize,user::User>>>)->Register
    {
        Register{
            db_mgr,
            user_map
        }
    }
}
#[async_trait]
impl SubHandle for Register
{
    type ABClient = AbClient;
    type Id = usize;

    async fn handle(&self, data: &[u8], len: u32, ext: u32, clients: &Arc<Mutex<HashMap<Self::Id, Box<Self::ABClient>, RandomState>>>, id: Self::Id) -> Option<(Vec<u8>, u32)> where Self::Id: Copy {
        if ext != EXT_REGISTER { return None; }

        let s = String::from_utf8_lossy(data).to_string();
        if let Ok(u) = serde_json::from_str::<RegUser>(s.as_str())
        {
            if !Regex::new(r"^[\da-zA-Z_!,\.]{5,16}$").unwrap().is_match(u.acc.as_str())
            {
                return Some((vec![],EXT_ERR_BAD_ACCOUNT));
            }
            if !Regex::new(r"^[\da-zA-Z_!,\.]{8,16}$").unwrap().is_match(u.pwd.as_str())
            {
                return Some((vec![],EXT_ERR_BAD_PASSWORD));
            }
            if !Regex::new(r"^[\u4e00-\u9fa5\w]{5,12}$").unwrap().is_match(u.name.as_str())
            {
                return Some((vec![],EXT_ERR_BAD_USERNAME));
            }
            match self.db_mgr.get_conn()
            {
                Ok(mut c) => {

                    let has_user = match c.query_first::<User,_>(format!("select * from user where user.acc='{}'",u.acc)) {
                        Ok(Some(u)) => {
                            true
                        }
                        Err(e) => {
                            dbg!(e);
                            return Some((vec![],EXT_ERR_NOT_KNOW));
                        }
                        _ => {false}
                    };
                    if has_user { return Some((vec![],EXT_ERR_ACC_REGISTERED)); }
                    match c.exec_drop("insert into user (name,acc,pwd) values((?),(?),(?));",
                                   (u.name,u.acc,u.pwd))
                    {
                        Ok(_) => {
                            return Some((vec![],EXT_REGISTER));
                        }
                        Err(e) => {
                            dbg!(e);
                            return Some((vec![],EXT_ERR_NOT_KNOW));
                        }
                    }
                }
                Err(e) => {
                    dbg!(e);
                    return Some((vec![],EXT_ERR_NOT_KNOW));
                }
            }
        }else{
            return Some((vec![],EXT_ERR_PARSE_ARGS));
        }

        None
    }
}