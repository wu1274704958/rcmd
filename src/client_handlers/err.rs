use crate::extc::*;
use terminal::Color;
use rcmd_suit::client_handler::SubHandle;
use async_trait::async_trait;
pub struct Err{

}
#[async_trait]
impl SubHandle for Err
{
    async fn handle(&self, data: &[u8], len: u32, ext: u32) -> Option<(Vec<u8>, u32)> {
        
        let out = terminal::stdout();
        out.batch(terminal::Action::SetForegroundColor(Color::Red));
        out.flush_batch();
        println!("error code = {}",ext);
        out.batch(terminal::Action::SetForegroundColor(Color::Reset));
        out.flush_batch();
        
        None
    }

    fn interested(&self, ext:u32) ->bool {
        ext >= EXT_DEFAULT_ERR_CODE
    }
}