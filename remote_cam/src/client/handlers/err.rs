use crate::client::extc;
use rcmd_suit::client_handler::SubHandle;
use async_trait::async_trait;
use crate::context::GLOB_CXT;
pub struct Err{

}
#[async_trait]
impl SubHandle for Err
{
    async fn handle(&self, data: &[u8], len: u32, ext: u32) -> Option<(Vec<u8>, u32)> {
        if let Ok(mut c) = GLOB_CXT.lock()
        {
            c.on_err(ext);
        }
        None
    }
    fn interested(&self, ext:u32) ->bool {
        ext >= extc::EXT_DEFAULT_ERR_CODE
    }
}