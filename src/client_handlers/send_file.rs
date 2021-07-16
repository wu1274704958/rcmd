use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use crate::extc::*;
use rcmd_suit::tools::{TOKEN_BEGIN,TOKEN_END};
use std::mem::size_of;
use rcmd_suit::tools::{u32_form_bytes,set_slices_form_u32};
use rcmd_suit::client_handler::SubHandle;
use async_trait::async_trait;
pub struct SendFile
{

}

impl SendFile {
    pub fn new()->SendFile
    {
        SendFile{
        }
    }
}
#[async_trait]
impl SubHandle for SendFile
{
    async fn handle(&self, data: &[u8], len: u32, ext: u32) -> Option<(Vec<u8>, u32)> {
        match ext
        {
            EXT_SAVE_FILE_RET |
            EXT_SAVE_FILE_CREATE_RET =>{
                let mid = data.len() - size_of::<u32>();
                let len = u32_form_bytes(&data[mid..]);
                let name = String::from_utf8_lossy(&data[0..mid]).to_string();
                println!("{} send {} bytes",name,len);
            }
            EXT_SAVE_FILE_ELF_RET => {
                let name = String::from_utf8_lossy(data).to_string();
                println!("{} send complete",name);
            }
            EXT_ERR_SAVE_FILE_RET_EXT
            => {
                let ext = u32_form_bytes(data);
                eprintln!("Send file ret error code = {}",ext);
            }
            // EXT_SEND_FILE |
            // EXT_SEND_FILE_ELF |
            // EXT_SEND_FILE_CREATE  => {
            //     println!("ext {} forward success!",ext);
            // }
            _ => {}
        }
        None
    }

    fn interested(&self, ext:u32) ->bool {
        ext == EXT_SAVE_FILE_RET ||
        ext == EXT_SAVE_FILE_CREATE_RET ||
        ext == EXT_SAVE_FILE_ELF_RET ||
        ext == EXT_ERR_SAVE_FILE_RET_EXT 
    }
}