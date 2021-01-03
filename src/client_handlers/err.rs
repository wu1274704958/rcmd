use crate::client_handlers::def_handler::SubHandle;
use crate::ext_code::*;
pub struct Err{

}

impl SubHandle for Err
{
    fn handle(&self, data: &[u8], len: u32, ext: u32) -> Option<(Vec<u8>, u32)> {
        if ext >= EXT_DEFAULT_ERR_CODE{
            println!("error code = {}",ext);
        }
        None
    }
}