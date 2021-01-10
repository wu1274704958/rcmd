use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use crate::model::user;
use crate::handler::SubHandle;
use crate::ab_client::AbClient;
use crate::ext_code::{EXT_SEND_FILE, EXT_ERR_PERMISSION_DENIED, EXT_ERR_PARSE_ARGS, EXT_ERR_NOT_FOUND_LID, EXT_SAVE_FILE, EXT_ERR_SAVE_FILE_RET_EXT};
use std::collections::hash_map::RandomState;
use std::mem::size_of;

pub struct SendFile{
    user_map:Arc<Mutex<HashMap<usize,user::User>>>
}


impl SendFile {
    pub fn new(user_map:Arc<Mutex<HashMap<usize,user::User>>>)->SendFile
    {
        SendFile{
            user_map
        }
    }
}


impl SubHandle for SendFile
{
    type ABClient = AbClient;
    type Id = usize;

    fn handle(&self, data: &[u8], len: u32, ext: u32, clients: &Arc<Mutex<HashMap<Self::Id, Box<Self::ABClient>, RandomState>>>, id: Self::Id) -> Option<(Vec<u8>, u32)> where Self::Id: Copy {
        if ext != EXT_SEND_FILE && ext != EXT_SAVE_FILE && ext != EXT_ERR_SAVE_FILE_RET_EXT{ return None;}
        if ext == EXT_SEND_FILE {
            let has_permission = if let Ok(u) = self.user_map.lock()
            {
                if let Some(user) = u.get(&id)
                {
                    user.super_admin
                } else { false }
            } else { false };
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

            if let Ok(mut cls) = clients.lock()
            {
                if let Some(cl) = cls.get_mut(&lid)
                {
                    cl.push_msg(data[size_of::<usize>()..].to_vec(), EXT_SAVE_FILE);
                } else {
                    return Some((vec![], EXT_ERR_NOT_FOUND_LID));
                }
            }
        }
        None
    }
}
