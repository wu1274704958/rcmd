use std::sync::{Arc};
use std::collections::HashMap;
use crate::model::user;
use rcmd_suit::handler::SubHandle;
use rcmd_suit::ab_client::AbClient;
use crate::extc::*;
use std::collections::hash_map::RandomState;
use std::mem::size_of;
use rcmd_suit::utils::temp_permission::TempPermission;
use tokio::sync::Mutex;
use async_trait::async_trait;


pub struct PullFile{
    user_map:Arc<Mutex<HashMap<usize,user::User>>>,
    temp_permission:TempPermission
}


impl PullFile {
    pub fn new(user_map:Arc<Mutex<HashMap<usize,user::User>>>,temp_permission:TempPermission)->PullFile
    {
        PullFile{
            user_map,
            temp_permission
        }
    }
}

#[async_trait]
impl SubHandle for PullFile
{
    type ABClient = AbClient;
    type Id = usize;

    async fn handle(&self, data: &[u8], len: u32, ext: u32, clients: &Arc<Mutex<HashMap<Self::Id, Box<Self::ABClient>, RandomState>>>, id: Self::Id) -> Option<(Vec<u8>, u32)> where Self::Id: Copy {
        match ext
        {
            EXT_PULL_FILE_S => {
                let has_permission = {
                    let u = self.user_map.lock().await;
                    if let Some(user) = u.get(&id)
                    {
                        user.super_admin
                    } else { false }
                };
                if !has_permission {
                    return Some((vec![], EXT_ERR_PERMISSION_DENIED));
                }
                if data.len() < size_of::<usize>()
                {
                    return Some((vec![], EXT_ERR_PARSE_ARGS));
                }
                let mut buf = [0u8; size_of::<usize>()];
                buf.copy_from_slice(&data[0..size_of::<usize>()]);
                let lid = usize::from_be_bytes(buf);
                let mut id_buf = id.to_be_bytes();
                let mut cls = clients.lock().await;
                {
                    if let Some(cl) = cls.get_mut(&lid)
                    {
                        let mut v = Vec::with_capacity(data.len());
                        v.resize(data.len(), 0);
                        (&mut v[0..size_of::<usize>()]).copy_from_slice(&id_buf);
                        (&mut v[size_of::<usize>()..]).copy_from_slice(&data[size_of::<usize>()..]);
                        cl.push_msg(v, EXT_PULL_FILE_C);
                        self.temp_permission.give_permission(id,lid);
                        return Some((vec![], ext));
                    } else {
                        return Some((vec![], EXT_ERR_NOT_FOUND_LID));
                    }
                }
            }
            EXT_PULL_FILE_C |
            EXT_ERR_PULL_FILE_RET_EXT => {
                let mut id_buf = [0u8;size_of::<usize>()];
                id_buf.copy_from_slice(&data[0..size_of::<usize>()]);
                let lid = usize::from_be_bytes(id_buf);
                self.temp_permission.take_permission(lid,id);
                let mut cls = clients.lock().await;
                {
                    if let Some(cl) = cls.get_mut(&lid)
                    {
                        cl.push_msg(data[size_of::<usize>()..].to_vec(),ext);
                    }
                }
            }
            _=>{}
        }
        None
    }

    fn interested(&self, ext:u32) ->bool {
        ext == EXT_PULL_FILE_S || ext == EXT_PULL_FILE_C || ext == EXT_ERR_PULL_FILE_RET_EXT
    }
}