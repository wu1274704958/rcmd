use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use std::mem::size_of;
use tokio::sync::Mutex;
use rcmd_suit::ab_client::AbClient;
use async_trait::async_trait;
use rcmd_suit::handler::SubHandle;
use std::sync::Arc;
use crate::extc;
use crate::model::user;

pub struct AgentCmdForward
{
    user_map:Arc<Mutex<HashMap<usize,user::User>>>,
}
impl AgentCmdForward {
    pub fn new(user_map:Arc<Mutex<HashMap<usize,user::User>>>)->AgentCmdForward {
        AgentCmdForward{user_map}
    }

    fn new_data_with(&self,cp:usize,after: &[u8]) -> Vec<u8>
    {
        let mut d = Vec::<u8>::new();
        let slice = cp.to_be_bytes();
        d.extend_from_slice(slice.as_slice());
        d.extend_from_slice(&after[slice.len()..]);
        d
    }
    fn map_ack(req:u32) -> u32
    {
        match req {
            extc::EXT_AGENT_GET_DATA_CS|
            extc::EXT_AGENT_RET_DATA_CS|
            extc::EXT_AGENT_EXEC_CMD_CS|
            extc::EXT_AGENT_SET_DATA_CS => {req - 4}
            extc::EXT_ERR_CALL_AGENT_METHOD_CS |
            extc::EXT_ERR_LOSE_AGENT_CONTEXT_CS => { req - 2 }
            _=>{0}
        }        
    }
}
#[async_trait]
impl SubHandle for AgentCmdForward
{
    type ABClient = AbClient;
    type Id = usize;

    async fn handle(&self, data: &[u8], _len: u32, ext: u32, clients: &Arc<Mutex<HashMap<Self::Id, Box<Self::ABClient>, RandomState>>>, id: Self::Id) -> Option<(Vec<u8>,u32)> where Self::Id: Copy {
        let mut buf = [0u8; size_of::<usize>()];
        buf.copy_from_slice(&data[0..size_of::<usize>()]);
        let lid = usize::from_be_bytes(buf);

        let has_permission = {
            let u = self.user_map.lock().await;
            if let Some(user) = u.get(&id)
            {
                user.super_admin
            } else { false }
        };
        if !has_permission {
            return Some((vec![], extc::EXT_ERR_PERMISSION_DENIED));
        }
        let mut cls = clients.lock().await;
        {
            if let Some(cl) = cls.get_mut(&lid)
            {
                cl.push_msg(self.new_data_with(id, data), Self::map_ack(ext));
                return Some((vec![], ext));
            } else {
                return Some((vec![], extc::EXT_ERR_NOT_FOUND_LID));
            }
        }
        None
    }

    fn interested(&self, ext:u32) ->bool {
        ext == extc::EXT_AGENT_GET_DATA_CS||
        ext == extc::EXT_AGENT_RET_DATA_CS||
        ext == extc::EXT_AGENT_EXEC_CMD_CS||
        ext == extc::EXT_AGENT_SET_DATA_CS||
        ext == extc::EXT_ERR_CALL_AGENT_METHOD_CS ||
        ext == extc::EXT_ERR_LOSE_AGENT_CONTEXT_CS
    }
}