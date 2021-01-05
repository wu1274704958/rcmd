use crate::client_handlers::def_handler::SubHandle;
use async_std::sync::Arc;
use std::sync::Mutex;
use crate::utils::rcmd::*;
use crate::ext_code::*;
use crate::model;
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
            let res = serde_json::from_str::<model::SendMsg>(s.as_str());
            match res{
                Ok(msg) => {
                    println!("from target {}",msg.lid);
                    let result = serde_json::from_str::<CmdRes>(&msg.msg);
                    match result{
                        Ok(res) => {
                            println!("out:\n{}err:\n{}",res.out,res.err);
                            if res.code.is_some() {
                                println!("code:{}",res.code.unwrap());
                            }
                        }
                        Err(e) =>{
                            println!("run cmd parse result error {:?}",e);
                        }
                    }
                }
                Err(e) =>{
                    println!("run cmd parse error {:?}",e);
                }
            }
        }
        None
    }
}