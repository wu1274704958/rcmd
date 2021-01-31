use tokio::sync::Mutex;
use std::collections::HashMap;
use rcmd_suit::handler::SubHandle;
use std::collections::hash_map::RandomState;
use rcmd_suit::ab_client::AbClient;
use crate::extc::{EXT_RUN_CMD, EXT_EXEC_CMD, EXT_ERR_NOT_LOGIN, EXT_ERR_PERMISSION_DENIED, EXT_ERR_PARSE_ARGS, EXT_ERR_NOT_KNOW, EXT_ERR_NOT_FOUND_LID, EXT_ERR_EXEC_CMD_RET_ERR};
use std::ops::AddAssign;
use crate::model::user;
use crate::model;
use rcmd_suit::tools;
use async_trait::async_trait;
use std::sync::Arc;

pub enum ExecState{
    Init,
    SendToTarget,
    Received,
    SendToSu
}

pub struct ExecData{
    pub tar:usize,
    pub su:usize,
    pub st:ExecState
}

impl ExecData{
    pub fn new(tar:usize,su:usize)->ExecData
    {
        ExecData{
            tar,
            su,
            st:ExecState::Init
        }
    }
}

pub struct ExecCmd{
    user_map:Arc<Mutex<HashMap<usize,user::User>>>,
    logic_id:Arc<Mutex<usize>>,
    data:Arc<Mutex<HashMap<usize,ExecData>>>
}

impl ExecCmd{
    pub fn new(user_map:Arc<Mutex<HashMap<usize,user::User>>>)->ExecCmd{
        ExecCmd{
            user_map,
            logic_id : Arc::new(Mutex::new(0)),
            data : Arc::new(Mutex::new(HashMap::new()))
        }
    }

    pub async fn next_id(&self) ->usize
    {
        let mut l = self.logic_id.lock().await;
        l.add_assign(1);
        return *l;
    }

    pub async fn del_data(&self,i:usize) -> Option<ExecData>
    {
        let mut d = self.data.lock().await;
        {
            let res = if d.contains_key(&i) {
                d.remove(&i)
            }else{None};
            if d.len() == 0
            {
                let mut l = self.logic_id.lock().await;
                *l = 0;
            }
            res
        }
    }

    pub async fn set_st(&self,i:usize,st:ExecState)
    {
        let mut d = self.data.lock().await;
        {
            if d.contains_key(&i) {
                d.get_mut(&i).unwrap().st = st;
            }
        }
    }

    pub async fn push(&self,i:usize,data:ExecData)
    {
        let mut d = self.data.lock().await;
        {
            d.insert(i,data);
        }
    }
}

#[async_trait]
impl SubHandle for ExecCmd
{
    type ABClient = AbClient;
    type Id = usize;

    async fn handle(&self, data: &[u8], len: u32, ext: u32, clients: &Arc<Mutex<HashMap<Self::Id, Box<Self::ABClient>, RandomState>>>, id: Self::Id) -> Option<(Vec<u8>, u32)> where Self::Id: Copy {
        if ext == EXT_RUN_CMD{
            let is_su = {
                let um = self.user_map.lock().await;
                {
                    if let Some(u) = um.get(&id) {
                        u.super_admin
                    } else {
                        return Some((vec![], EXT_ERR_NOT_LOGIN));
                    }
                }
            };
            if !is_su { return Some((vec![],EXT_ERR_PERMISSION_DENIED)); }

            let s = String::from_utf8_lossy(data).to_string();
            if let Ok(msg) = serde_json::from_str::<model::SendMsg>(&s)
            {
                let mut cls = clients.lock().await;
                {
                    if let Some(cl) = cls.get_mut(&msg.lid){
                        let sm = model::SendMsg {
                            lid: self.next_id().await,
                            msg: msg.msg.clone()
                        };
                        let mut st = ExecData::new(msg.lid.clone(),id.clone());
                        st.st = ExecState::SendToTarget;
                        self.push(sm.lid,st).await;
                        cl.push_msg(serde_json::to_string(&sm).unwrap().into_bytes(),EXT_EXEC_CMD);
                    }else{ return Some((vec![],EXT_ERR_NOT_FOUND_LID));}
                }
            }else{ return Some((vec![],EXT_ERR_PARSE_ARGS));}

        }else if ext == EXT_EXEC_CMD{
            let s = String::from_utf8_lossy(data).to_string();
            let msg = serde_json::from_str::<model::SendMsg>(&s);
            match msg {
                Err(e) => {
                    println!("parse exec cmd return msg failed! {:?}",e);
                }
                Ok(mut m) =>{
                    if let Some(ed) = self.del_data(m.lid).await{
                        let mut cls = clients.lock().await;
                        if let Some(cl) = cls.get_mut(&ed.su)
                        {
                            m.lid = ed.tar;
                            cl.push_msg(serde_json::to_string(&m).unwrap().into_bytes(),EXT_RUN_CMD);
                        }
                    }
                }
            }
        }else if ext == EXT_ERR_EXEC_CMD_RET_ERR{
            if data.len() == std::mem::size_of::<u32>(){
                let err = tools::u32_form_bytes(data);
                println!("exec cmd return error code {}",err);
            }
        }
        None
    }
}