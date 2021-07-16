use std::sync::Mutex;
use crate::extc::*;
use crate::model;
use terminal::Color;
use std::collections::VecDeque;
use std::mem::size_of;
use std::path::PathBuf;
use std::str::FromStr;
use std::convert::Infallible;
use std::fs::{OpenOptions, File};
use std::io::Read;
use std::ffi::OsStr;
use rcmd_suit::client_handler::SubHandle;
use rcmd_suit::tools::u32_form_bytes;
use async_trait::async_trait;
pub struct PullFileRet
{

}

impl PullFileRet {
    pub fn new()->PullFileRet
    {
        PullFileRet{
        }
    }
}
#[async_trait]
impl SubHandle for PullFileRet
{
    async fn handle(&self, data: &[u8], len: u32, ext: u32) -> Option<(Vec<u8>, u32)> {
        match ext
        {
            EXT_PULL_FILE_S => {
                println!("pull forward success!");
            }
            EXT_PULL_FILE_C => {
                println!("pull handle success!");
            }
            EXT_ERR_PULL_FILE_RET_EXT => {
                let ext = u32_form_bytes(data);
                eprintln!("pull file ret error code = {}",ext);
            }
            _ => {}
        }
        None
    }

    fn interested(&self, ext:u32) ->bool {
        EXT_PULL_FILE_S == ext || ext == EXT_PULL_FILE_C || ext == EXT_ERR_PULL_FILE_RET_EXT
    }
}