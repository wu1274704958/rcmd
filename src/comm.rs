use crate::extc::*;
use std::vec::Vec;


lazy_static! {
   pub static ref IGNORE_EXT: Vec<u32> = {
        vec![
            EXT_SEND_FILE_CREATE,EXT_SEND_FILE,EXT_SEND_FILE_ELF,
            EXT_SAVE_FILE_CREATE,EXT_SAVE_FILE,EXT_SAVE_FILE_ELF,
            EXT_SAVE_FILE_RET,EXT_SAVE_FILE_CREATE_RET,EXT_SAVE_FILE_ELF_RET,
            EXT_UPLOAD_FILE_CREATE,EXT_UPLOAD_FILE,EXT_UPLOAD_FILE_ELF,
            EXT_P2P_RELAY_MSG_SC,EXT_P2P_RELAY_MSG_CS
        ]
    };
}