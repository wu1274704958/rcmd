use crate::client_handlers::def_handler::SubHandle;
use async_std::sync::Arc;
use std::sync::Mutex;
use crate::utils::rcmd::*;
use crate::ext_code::*;
use crate::model;
use terminal::Color;

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
                            let out = terminal::stdout();
                            out.batch(terminal::Action::SetForegroundColor(Color::Green));
                            out.flush_batch();
                            println!("{}\n",res.out);
                            out.batch(terminal::Action::SetForegroundColor(Color::Red));
                            out.flush_batch();
                            println!("{}\n",res.err);

                            if res.code.is_some() {
                                out.batch(terminal::Action::SetForegroundColor(Color::Blue));
                                out.flush_batch();
                                println!("code:{}",res.code.unwrap());
                            }
                            out.batch(terminal::Action::SetForegroundColor(Color::Reset));
                            out.flush_batch();
                        }
                        Err(e) =>{
                            let out = terminal::stdout();
                            out.batch(terminal::Action::SetForegroundColor(Color::Red));
                            out.flush_batch();
                            println!("run cmd parse result error {:?}",e);
                            out.batch(terminal::Action::SetForegroundColor(Color::Reset));
                            out.flush_batch();
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