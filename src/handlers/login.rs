use rcmd_suit::ab_client::AbClient;
use crate::model::user;
use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use tokio::sync::Mutex;
use crate::extc::{EXT_LOGIN, EXT_LOGOUT, EXT_ERR_PARSE_ARGS, EXT_ERR_ALREADY_LOGIN, EXT_ERR_NOT_KNOW, EXT_ERR_WRONG_PASSWORD, EXT_ERR_NOT_FOUND_ACC, EXT_ERR_NOT_LOGIN};
use rcmd_suit::db::db_mgr::DBMgr;
use crate::model::user::{User, MinUser};
use mysql::prelude::Queryable;
use rcmd_suit::plug::Plug;
use rcmd_suit::config_build::Config;
use rcmd_suit::utils::temp_permission::TempPermission;
use async_trait::async_trait;
use std::sync::Arc;
use rcmd_suit::handler::SubHandle;

pub struct Login{
    db_mgr:Arc<DBMgr>,
    login_map:Arc<Mutex<HashMap<String,usize>>>,
    user_map:Arc<Mutex<HashMap<usize,user::User>>>
}

impl Login {
    pub fn new(db_mgr:Arc<DBMgr>,user_map:Arc<Mutex<HashMap<usize,user::User>>>,login_map:Arc<Mutex<HashMap<String,usize>>>) -> Login
    {
        Login{db_mgr,login_map,user_map}
    }
}

#[async_trait]
impl SubHandle for Login
{
    type ABClient = AbClient;
    type Id = usize;

    async fn handle(&self, data: &[u8], _len: u32, ext: u32, _clients: &Arc<Mutex<HashMap<Self::Id, Box<Self::ABClient>, RandomState>>>, id: Self::Id) -> Option<(Vec<u8>, u32)> where Self::Id: Copy {
        if ext == EXT_LOGIN{
            {
                let lm = self.user_map.lock().await;
                if let Some(_u) = lm.get(&id)
                {
                    return Some((vec![],EXT_ERR_ALREADY_LOGIN));
                }
            }
            let s = String::from_utf8_lossy(data).to_string();
            if let Ok(mu) = serde_json::from_str::<MinUser>(s.as_str()) {
                {
                    let lm = self.login_map.lock().await;
                    if let Some(_id) = lm.get(&mu.acc)
                    {
                        return Some((vec![], EXT_ERR_ALREADY_LOGIN));
                    }
                }

                if let Ok(mut c) = self.db_mgr.get_conn()
                {
                    if let Ok(Some(Ok(mut user))) = c.query_first_opt::<User,_>(format!("select * from user where user.acc='{}';",mu.acc))
                    {
                        if user.pwd != mu.pwd
                        {
                            return Some((vec![], EXT_ERR_WRONG_PASSWORD));
                        }
                        println!("insert login map");
                        {
                            let mut m = self.login_map.lock().await;
                            m.insert(mu.acc, id.clone());
                        }
                        println!("insert user map");
                        {
                            let mut m = self.user_map.lock().await;
                            m.insert(id, user.clone());
                        }
                        println!("return the user info");
                        user.pwd.clear();
                        let s = serde_json::to_string(&user).unwrap();
                        return Some((s.into_bytes(),EXT_LOGIN));
                    }else{
                        return Some((vec![],EXT_ERR_NOT_FOUND_ACC));
                    }
                }else {
                    return Some((vec![], EXT_ERR_NOT_KNOW));
                }

            }else{
                return Some((vec![],EXT_ERR_PARSE_ARGS));
            }
        }else if ext == EXT_LOGOUT{
            let mut m = self.user_map.lock().await;
            {
                if !m.contains_key(&id)
                {
                   return Some((vec![],EXT_ERR_NOT_LOGIN));
                }else{
                    println!("remove user");
                    let u = m.remove(&id).unwrap();
                    let acc = u.acc.clone();
                    if let Ok(mut c) = self.db_mgr.get_conn()
                    {
                        match c.exec_drop("UPDATE user SET name = (?), acc = (?) ,pwd = (?) ,is_admin = (?) , super_admin = (?)
                            WHERE user.id = (?);",
                                       (u.name,u.acc,u.pwd,u.is_admin,u.super_admin,u.id))
                        {
                            Ok(_l) =>{
                                println!("remove login");
                                {
                                    let mut m = self.login_map.lock().await;
                                    m.remove(&acc);
                                }
                                return Some((vec![], EXT_LOGOUT));
                            }
                            Err(e)=>{
                                dbg!(e);
                                return Some((vec![], EXT_ERR_NOT_KNOW));
                            }
                        }
                    }else{
                        return Some((vec![], EXT_ERR_NOT_KNOW));
                    }
                }
            }
        }
        None
    }
    fn interested(&self, ext:u32) ->bool {
        ext == EXT_LOGIN || ext == EXT_LOGOUT
    }
}


pub struct OnDeadPlug{
    db_mgr:Arc<DBMgr>,
    login_map:Arc<Mutex<HashMap<String,usize>>>,
    user_map:Arc<Mutex<HashMap<usize,user::User>>>,
    temp_permission:TempPermission
}

impl OnDeadPlug{
    pub fn new(db_mgr:Arc<DBMgr>,user_map:Arc<Mutex<HashMap<usize,user::User>>>,
               login_map:Arc<Mutex<HashMap<String,usize>>>,
               temp_permission:TempPermission) -> Self
{
    OnDeadPlug{db_mgr,login_map,user_map,temp_permission}
}
}
#[async_trait]
impl Plug for OnDeadPlug {
    type ABClient = AbClient;
    type Id = usize;
    type Config = Config;

    async fn run(&self, id: Self::Id, _clients: &Arc<Mutex<HashMap<Self::Id, Box<Self::ABClient>, RandomState>>>, _config: Arc<Self::Config>) where Self::Id: Copy {
        let mut m = self.user_map.lock().await;
        {
            if m.contains_key(&id){
                println!("remove user");
                let u = m.remove(&id).unwrap();
                let acc = u.acc.clone();
                if let Ok(mut c) = self.db_mgr.get_conn()
                {
                    match c.exec_drop("UPDATE user SET name = (?), acc = (?) ,pwd = (?) ,is_admin = (?) , super_admin = (?)
                            WHERE user.id = (?);",
                                      (u.name,u.acc,u.pwd,u.is_admin,u.super_admin,u.id))
                    {
                        Ok(_l) =>{
                            println!("remove login");
                            {
                                let mut m = self.login_map.lock().await;
                                m.remove(&acc);
                            }
                        }
                        Err(e)=>{
                            dbg!(e);
                        }
                    }
                }
            }
        }
        self.temp_permission.take_all_permission(id);
    }
}