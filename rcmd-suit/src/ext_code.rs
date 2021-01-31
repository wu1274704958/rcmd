
// ext 9        heartbeat
// ext 0        default
// ext 10 .. 20  asymmetric cryptographic
// ext 20 .. 40  asymmetric cryptographic err
// ext 40 41      upload file

pub const EXT_HEARTBEAT:u32 = 9;
pub const EXT_DEFAULT:u32 = 0;
pub const EXT_ASY_CRY_BEGIN:u32 = 10;
pub const EXT_ASY_CRY_END:u32 = 19;
pub const EXT_ASY_CRY_ERR_BEGIN:u32 = 20;
pub const EXT_ASY_CRY_ERR_GEN_KEY_UNKNOW:u32 = 38;
pub const EXT_ASY_CRY_ERR_END:u32 = 39;