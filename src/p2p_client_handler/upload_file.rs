use rcmd_suit::handler::{Handle, SubHandle};
use std::collections::HashMap;
use tokio::sync::Mutex;
use rcmd_suit::ab_client::AbClient;
use std::time::SystemTime;
use crate::extc::*;
use rcmd_suit::tools::{TOKEN_BEGIN, TOKEN_END};
use tokio::fs::{File, OpenOptions};
use std::io::Write;
use async_trait::async_trait;
use tokio::prelude::io::AsyncWriteExt;
use std::sync::Arc;
use rcmd_suit::client_handler;
use rcmd_suit::utils::stream_parser::Stream;
use rcmd_suit::utils::stream_parser::StreamParse;

pub struct UploadHandler
{
    file_map:Arc<Mutex<HashMap<String,(usize,File)>>>
}

impl UploadHandler {
    pub fn new()->UploadHandler
    {
        UploadHandler{
            file_map:Arc::new(Mutex::new(HashMap::new())),
        }
    }
}
#[async_trait]
impl SubHandle for UploadHandler
{
    type ABClient = AbClient;
    type Id = usize;

    async fn handle(&self, data: &[u8], len: u32, ext: u32, clients: &Arc<Mutex<HashMap<Self::Id, Box<Self::ABClient>>>>, id: Self::Id) -> Option<(Vec<u8>,u32)> where Self::Id: Copy {

        //println!("{:?} ext: {}",data,ext);
        handle_comm(data,self,id,ext).await
    }

    fn interested(&self, ext:u32) ->bool {
        ext == EXT_UPLOAD_FILE_CREATE || ext == EXT_UPLOAD_FILE || ext == EXT_UPLOAD_FILE_ELF
        || ext == EXT_UPLOAD_FILE_CREATE_BACK || ext == EXT_UPLOAD_FILE_BACK || ext == EXT_UPLOAD_FILE_ELF_BACK
    }
}

#[async_trait]
impl client_handler::SubHandle for UploadHandler{
    async fn handle(&self, data: &[u8], len: u32, ext: u32) -> Option<(Vec<u8>, u32)> {
        handle_comm(data,self,1,ext).await
    }

    fn interested(&self, ext: u32) -> bool {
        ext == EXT_UPLOAD_FILE_CREATE || ext == EXT_UPLOAD_FILE || ext == EXT_UPLOAD_FILE_ELF
            || ext == EXT_UPLOAD_FILE_CREATE_BACK || ext == EXT_UPLOAD_FILE_BACK || ext == EXT_UPLOAD_FILE_ELF_BACK
    }
}

async fn handle_comm(data: &[u8],this:&UploadHandler,id:usize,ext:u32) -> Option<(Vec<u8>,u32)>
{
    match ext {
        EXT_UPLOAD_FILE_CREATE_BACK|
        EXT_UPLOAD_FILE_ELF_BACK |
        EXT_UPLOAD_FILE_BACK =>{
            let mut s = Stream::new(data);
            if let Some(len) = u32::stream_parse(&mut s)
            {
                let a = String::from_utf8_lossy( s.get_rest());
                if ext == EXT_UPLOAD_FILE_CREATE_BACK { println!("开始接收 {}",a);}
                else if ext == EXT_UPLOAD_FILE_ELF_BACK {
                    println!("接收完成 {}",a);
                }
            }
            return None;
        }
        _=>{}
    }

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
            let m = this.file_map.lock().await;
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

        if let Ok(mut f) = OpenOptions::new().create(true).append(false).write(true).open(name.clone()).await
        {
            let buf = &data[(name_e+1)..];
            if let Ok(()) = f.write_all(buf).await
            {
                let b = (buf.len() as u32).to_be_bytes();
                rd.extend_from_slice(b.as_ref());
                return {
                    let mut fm = this.file_map.lock().await;
                    {
                        fm.insert(name.clone(), (id, f));
                        Some((rd, EXT_UPLOAD_FILE_CREATE_BACK))
                    }
                };
            }else{
                return Some((rd,EXT_ERR_WRITE_FILE_FAILED));
            }
        }else{
            return Some((rd,EXT_ERR_CREATE_FILE_FAILED));
        }
    } else if ext == EXT_UPLOAD_FILE{

        let mut fm = this.file_map.lock().await;
        {
            if let Some(f) = fm.get_mut(&name)
            {
                if f.0 != id
                {
                    return Some((rd, EXT_ERR_NO_ACCESS_PERMISSION));
                }
                let file_data = &data[(name_e.clone()+1)..];
                if let Ok(()) = f.1.write_all(file_data).await
                {
                    let b = (file_data.len() as u32).to_be_bytes();
                    rd.extend_from_slice(b.as_ref());
                    //f.1.sync_all().unwrap();
                    return Some((rd,EXT_UPLOAD_FILE_BACK));
                }else{
                    return Some((rd,EXT_ERR_WRITE_FILE_FAILED));
                }
            }else { return Some((rd,EXT_ERR_FILE_NAME_NOT_EXITS)); }
        }

    }else if ext == EXT_UPLOAD_FILE_ELF{
        let mut fm = this.file_map.lock().await;
        {
            if let Some(f) = fm.get_mut(&name)
            {
                if f.0 != id
                {
                    return Some((rd, EXT_ERR_NO_ACCESS_PERMISSION));
                }
                f.1.sync_all().await.unwrap();
            }else{
                return Some((rd,EXT_ERR_FILE_NAME_NOT_EXITS));
            }

            if let Some(k) = fm.remove(&name)
            {
                return Some((rd,EXT_UPLOAD_FILE_ELF_BACK));
            }
        }
    }

    Some((rd,EXT_DEFAULT_ERR_CODE))
}