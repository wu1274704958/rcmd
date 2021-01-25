use crate::handler::{Handle, SubHandle};
use std::collections::hash_map::RandomState;
use async_std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::Mutex;
use crate::ab_client::AbClient;
use std::time::SystemTime;
use crate::ext_code::*;
use crate::tools::{TOKEN_BEGIN, TOKEN_END};
use std::fs::{File, OpenOptions};
use std::io::Write;
use crate::model::user;
use async_trait::async_trait;

pub struct UploadHandler
{
    file_map:Arc<Mutex<HashMap<String,(usize,File)>>>,
    user_map:Arc<Mutex<HashMap<usize,user::User>>>
}

impl UploadHandler {
    pub fn new(user_map:Arc<Mutex<HashMap<usize,user::User>>>)->UploadHandler
    {
        UploadHandler{
            file_map:Arc::new(Mutex::new(HashMap::new())),
            user_map
        }
    }
}
#[async_trait]
impl SubHandle for UploadHandler
{
    type ABClient = AbClient;
    type Id = usize;

    async fn handle(&self, data: &[u8], len: u32, ext: u32, clients: &Arc<Mutex<HashMap<Self::Id, Box<Self::ABClient>, RandomState>>>, id: Self::Id) -> Option<(Vec<u8>,u32)> where Self::Id: Copy {
        if ext != EXT_UPLOAD_FILE_CREATE && ext != EXT_UPLOAD_FILE && ext != EXT_UPLOAD_FILE_ELF {return  None;}
        let has_permission = {
            let u = self.user_map.lock().await;
            {
                if let Some(user) = u.get(&id)
                {
                    user.super_admin
                } else { false }
            }
        };
        if !has_permission{
            return Some((vec![],EXT_ERR_PERMISSION_DENIED));
        }
        //println!("{:?} ext: {}",data,ext);
        if data[0] != TOKEN_BEGIN
        {
            return Some((vec![],EXT_AGREEMENT_ERR_CODE));
        }

        let mut name_e = 0usize;
        loop  {
            if name_e >= data.len()
            {
                return Some((vec![],EXT_AGREEMENT_ERR_CODE));
            }else if *data.get(name_e.clone()).unwrap() == TOKEN_END
            {
                break;
            }
            name_e = name_e + 1;
        }
        let name = String::from_utf8_lossy(&data[1..name_e]).trim().to_string();

        if name.is_empty()
        {
            return Some((vec![],EXT_ERR_FILE_NAME_EMPTY));
        }

        let mut rd = name.as_bytes().to_vec();

        if ext == EXT_UPLOAD_FILE_CREATE{
            {
                let m = self.file_map.lock().await;
                {
                    if let Some(v) = m.get(&name)
                    {
                        if v.0 != id
                        {
                            return Some((rd, EXT_ERR_NO_ACCESS_PERMISSION));
                        } else {
                            return Some((rd, EXT_ERR_ALREADY_CREATED));
                        }
                    }
                }
            }

            if let Ok(mut f) = OpenOptions::new().create(true).append(false).write(true).open(name.clone())
            {
                if let Ok(l) = f.write(&data[(name_e+1)..])
                {
                    let b = (l as u32).to_be_bytes();
                    b.iter().for_each(|it|{rd.push(*it)});
                    return {
                        let mut fm = self.file_map.lock().await;
                        {
                            fm.insert(name.clone(), (id, f));
                            Some((rd, EXT_UPLOAD_FILE_CREATE))
                        }
                    };
                }else{
                    return Some((rd,EXT_ERR_WRITE_FILE_FAILED));
                }
            }else{
                return Some((rd,EXT_ERR_CREATE_FILE_FAILED));
            }
        } else if ext == EXT_UPLOAD_FILE{

            let mut fm = self.file_map.lock().await;
            {
                if let Some(f) = fm.get_mut(&name)
                {
                    if f.0 != id
                    {
                        return Some((rd, EXT_ERR_NO_ACCESS_PERMISSION));
                    }
                    let file_data = &data[(name_e.clone()+1)..];
                    if let Ok(l) = f.1.write(file_data)
                    {
                        let b = (l as u32).to_be_bytes();
                        b.iter().for_each(|it|{rd.push(*it)});
                        //f.1.sync_all().unwrap();
                        return Some((rd,EXT_UPLOAD_FILE));
                    }else{
                        return Some((rd,EXT_ERR_WRITE_FILE_FAILED));
                    }
                }else { return Some((rd,EXT_ERR_FILE_NAME_NOT_EXITS)); }
            }

        }else if ext == EXT_UPLOAD_FILE_ELF{
            let mut fm = self.file_map.lock().await;
            {
                if let Some(f) = fm.get_mut(&name)
                {
                    if f.0 != id
                    {
                        return Some((rd, EXT_ERR_NO_ACCESS_PERMISSION));
                    }
                    f.1.sync_all().unwrap();
                }else{
                    return Some((rd,EXT_ERR_FILE_NAME_NOT_EXITS));
                }

                if let Some(k) = fm.remove(&name)
                {
                    return Some((rd,EXT_UPLOAD_FILE_ELF));
                }
            }
        }
        Some((rd,EXT_DEFAULT_ERR_CODE))
    }
}