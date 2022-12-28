use std::{ mem::size_of};

use crate::{client::extc, context::Context};
use rcmd_suit::client_handler::SubHandle;
use async_trait::async_trait;
use crate::context::GLOB_CXT;
pub struct AgentExecuor{
    
}

impl AgentExecuor {
    fn new_data_with(&self,cp:usize,after:Vec<u8>) -> Vec<u8>
    {
        let mut d = Vec::<u8>::new();
        let slice = cp.to_be_bytes();
        d.extend_from_slice(slice.as_slice());
        d.extend_from_slice(&after[..]);
        d
    }
    fn new_data(&self,cp:usize) -> Vec<u8>
    {
        let mut d = Vec::<u8>::new();
        let slice = cp.to_be_bytes();
        d.extend_from_slice(slice.as_slice());
        d
    }
}

#[async_trait]
impl SubHandle for AgentExecuor
{
    async fn handle(&self, mut data: &[u8], _len: u32, ext: u32) -> Option<(Vec<u8>, u32)> {
        let mut buf = [0u8; size_of::<usize>()];
        buf.copy_from_slice(&data[0..size_of::<usize>()]);
        let lid = usize::from_be_bytes(buf);

        data = &data[size_of::<usize>()..];
        match ext {
            extc::EXT_AGENT_GET_DATA => {
                self.send_agent_data(lid)
            },
            extc::EXT_AGENT_EXEC_CMD => {
                self.exec(data,lid)
            }
            extc::EXT_AGENT_SET_DATA => {
                self.set_agent_data(data,lid)
            }
            _=>{None}
        }
    }


    fn interested(&self, ext:u32) ->bool {
        ext == extc::EXT_AGENT_GET_DATA || 
        ext == extc::EXT_AGENT_EXEC_CMD ||
        ext == extc::EXT_AGENT_SET_DATA
    }
}

impl AgentExecuor {
    fn send_agent_data(&self,lid:usize) -> Option<(Vec<u8>, u32)>
    {
        if let Ok(c) = GLOB_CXT.lock()
        {
            match c.get_agent_data(){
                Ok(d) => {
                    Some((self.new_data_with(lid, d.into_bytes()),extc::EXT_AGENT_RET_DATA_CS))
                }
                Err(e) =>{
                    toast(&c,format!("call get_agent_data err = {:?}",e),-1);
                    Some((self.new_data(lid),extc::EXT_ERR_CALL_AGENT_METHOD_CS))
                }
            }
        }else {
            Some((self.new_data(lid),extc::EXT_ERR_LOSE_AGENT_CONTEXT_CS))
        }
    }
    fn exec(&self,cmd:&[u8],lid:usize) -> Option<(Vec<u8>, u32)>
    {
        if let Ok(c) = GLOB_CXT.lock()
        {
            let cmd = String::from_utf8_lossy(cmd).to_string();
            match c.exec_cmd(&cmd){
                Ok(_) => {
                    drop(c);
                    self.send_agent_data(lid)
                }
                Err(e) =>{
                    toast(&c,format!("call exec_cmd err = {:?}",e),-1);
                    Some((self.new_data(lid),extc::EXT_ERR_CALL_AGENT_METHOD_CS))
                }
            }
        }else {
            Some((vec![],extc::EXT_ERR_LOSE_AGENT_CONTEXT))
        }
    }
    fn set_agent_data(&self,cmd:&[u8],lid:usize) -> Option<(Vec<u8>, u32)>
    {
        if let Ok(c) = GLOB_CXT.lock()
        {
            let cmd = String::from_utf8_lossy(cmd).to_string();
            let cmds: Vec<&str> = cmd.split(" ").map(|it| { it.trim() }).collect();
            let mut i = 0;
            loop {
                if i >= cmds.len() {break;}
                if let Err(e) = c.set_agent_data(cmds[i], cmds[i+1]) {
                    toast(&c,format!("call set_agent_data err = {:?}",e),-1);
                    return Some((self.new_data(lid),extc::EXT_ERR_CALL_AGENT_METHOD_CS));
                } 
                i += 2;
            }
            drop(c);
            self.send_agent_data(lid)
        }else {
            Some((self.new_data(lid),extc::EXT_ERR_LOSE_AGENT_CONTEXT_CS))
        }
    }
}

fn toast(c:&std::sync::MutexGuard<Context>,s:String,p:i32)
{
    c.toast(s.as_str(), p);
}