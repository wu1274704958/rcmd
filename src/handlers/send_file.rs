use std::sync::{Arc};
use tokio::sync::Mutex;
use std::collections::HashMap;
use crate::model::user;
use rcmd_suit::handler::SubHandle;
use rcmd_suit::ab_client::AbClient;
use crate::extc::*;
use std::collections::hash_map::RandomState;
use std::mem::size_of;
use rcmd_suit::utils::temp_permission::TempPermission;
use async_trait::async_trait;

pub struct SendFile{
    user_map:Arc<Mutex<HashMap<usize,user::User>>>,
    temp_permission:TempPermission
}


impl SendFile {
    pub fn new(user_map:Arc<Mutex<HashMap<usize,user::User>>>,temp_permission:TempPermission)->SendFile
    {
        SendFile{
            user_map,
            temp_permission
        }
    }
}

#[async_trait]
impl SubHandle for SendFile
{
    type ABClient = AbClient;
    type Id = usize;

    async fn handle(&self, data: &[u8], len: u32, ext: u32, clients: &Arc<Mutex<HashMap<Self::Id, Box<Self::ABClient>, RandomState>>>, id: Self::Id) -> Option<(Vec<u8>, u32)> where Self::Id: Copy {

        match ext
        {
            EXT_SEND_FILE_CREATE |
            EXT_SEND_FILE |
            EXT_SEND_FILE_ELF =>{
                let mut buf = [0u8; size_of::<usize>()];
                buf.copy_from_slice(&data[0..size_of::<usize>()]);
                let lid = usize::from_be_bytes(buf);

                let mut has_permission = {
                    let u = self.user_map.lock().await;
                    if let Some(user) = u.get(&id)
                    {
                        user.super_admin
                    } else { false }
                };
                if !has_permission {
                    has_permission = self.temp_permission.has_temp_permission(lid,id);
                }

                if !has_permission {
                    return Some((vec![], EXT_ERR_PERMISSION_DENIED));
                }
                if data.len() < size_of::<usize>()
                {
                    return Some((vec![], EXT_ERR_PARSE_ARGS));
                }

                let mut id_buf = id.to_be_bytes();
                let mut cls = clients.lock().await;
                {
                    if let Some(cl) = cls.get_mut(&lid)
                    {
                        let mut v = Vec::with_capacity(data.len());
                        v.resize(data.len(),0);
                        (&mut v[0..size_of::<usize>()]).copy_from_slice(&id_buf);
                        (&mut v[size_of::<usize>()..]).copy_from_slice(&data[size_of::<usize>()..]);
                        cl.push_msg(v, ext + 3);
                        return Some((vec![], ext));
                    } else {
                        return Some((vec![], EXT_ERR_NOT_FOUND_LID));
                    }
                }
            }

            EXT_SAVE_FILE |
            EXT_SAVE_FILE_ELF |
            EXT_SAVE_FILE_CREATE |
            EXT_ERR_SAVE_FILE_RET_EXT
            => {
                let mut id_buf = [0u8;size_of::<usize>()];
                id_buf.copy_from_slice(&data[0..size_of::<usize>()]);
                let id = usize::from_be_bytes(id_buf);
                let mut cls = clients.lock().await;
                {
                    if let Some(cl) = cls.get_mut(&id)
                    {
                        cl.push_msg(data[size_of::<usize>()..].to_vec(),map_ext(ext));
                    }
                }
            }
            _ => {}
        }
        None
    }

    fn interested(&self, ext:u32) ->bool {
        ext == EXT_SEND_FILE_CREATE ||
        ext == EXT_SEND_FILE ||
        ext == EXT_SEND_FILE_ELF ||
        ext == EXT_SAVE_FILE ||
        ext == EXT_SAVE_FILE_ELF ||
        ext == EXT_SAVE_FILE_CREATE ||
        ext == EXT_ERR_SAVE_FILE_RET_EXT
    }
}

fn map_ext(e:u32) -> u32
{
    match e {
        EXT_SAVE_FILE |
        EXT_SAVE_FILE_ELF |
        EXT_SAVE_FILE_CREATE => {
            e + 3
        }
        EXT_ERR_SAVE_FILE_RET_EXT => {
            e
        }
        _ => { e
        }
    }

}