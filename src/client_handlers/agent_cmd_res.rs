use crate::extc;
use rcmd_suit::client_handler::SubHandle;
use async_trait::async_trait;
pub struct AgentCmdRes{

}
#[async_trait]
impl SubHandle for AgentCmdRes
{
    async fn handle(&self, data: &[u8], len: u32, ext: u32) -> Option<(Vec<u8>, u32)> {
        let ret = String::from_utf8_lossy(data).to_string();
        println!("agent ret: {}",ret);
        None
    }

    fn interested(&self, ext:u32) ->bool {
        ext == extc::EXT_AGENT_RET_DATA
    }
}