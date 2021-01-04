use crate::client_handlers::def_handler::SubHandle;
use async_std::sync::Arc;
use std::sync::Mutex;
use crate::utils::rcmd::*;
use crate::ext_code::*;

pub struct RunCmd
{
}

impl RunCmd {
    pub fn new()->RunCmd
    {
        RunCmd{
        }
    }
}

impl SubHandle for RunCmd
{
    fn handle(&self, data: &[u8], len: u32, ext: u32) -> Option<(Vec<u8>, u32)> {
        if ext == EXT_RUN_CMD {
            let s = String::from_utf8_lossy(data).to_string();
            let res = serde_json::from_str::<CmdRes>(s.as_str()).unwrap();
            println!("out:\n{}err:\n{}",res.out,res.err);
            if res.code.is_some() {
                println!("code:{}",res.code.unwrap());
            }
        }
        None
    }
}