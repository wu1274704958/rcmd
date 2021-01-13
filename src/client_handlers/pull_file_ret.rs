use crate::client_handlers::def_handler::SubHandle;
use async_std::sync::Arc;
use std::sync::Mutex;
use crate::utils::rcmd::*;
use crate::ext_code::*;
use crate::model;
use terminal::Color;
use std::collections::VecDeque;
use std::mem::size_of;
use std::path::PathBuf;
use std::str::FromStr;
use std::convert::Infallible;
use crate::tools::{ext_content, TOKEN_BEGIN, TOKEN_END, u32_form_bytes};
use std::fs::{OpenOptions, File};
use async_std::io::Error;
use std::io::Read;
use std::ffi::OsStr;

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

impl SubHandle for PullFileRet
{
    fn handle(&self, data: &[u8], len: u32, ext: u32) -> Option<(Vec<u8>, u32)> {
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
}