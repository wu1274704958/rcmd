use crate::client_handlers::def_handler::SubHandle;
use crate::ext_code::*;
use terminal::Color;

pub struct Err{

}

impl SubHandle for Err
{
    fn handle(&self, data: &[u8], len: u32, ext: u32) -> Option<(Vec<u8>, u32)> {
        if ext >= EXT_DEFAULT_ERR_CODE{
            let out = terminal::stdout();
            out.batch(terminal::Action::SetForegroundColor(Color::Red));
            out.flush_batch();
            println!("error code = {}",ext);
            out.batch(terminal::Action::SetForegroundColor(Color::Reset));
            out.flush_batch();
        }
        None
    }
}