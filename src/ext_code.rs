
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
pub const EXT_ASY_CRY_ERR_END:u32 = 39;
pub const EXT_UPLOAD_FILE:u32 = 40;
pub const EXT_UPLOAD_FILE_ELF:u32 = 41;
pub const EXT_UPLOAD_FILE_CREATE:u32 = 42;
pub const EXT_DEFAULT_ERR_CODE:u32 = 50000;
pub const EXT_ERR_CREATE_FILE_FAILED:u32 = 50001;
pub const EXT_ERR_FILE_NAME_EMPTY:u32 = 50002;
pub const EXT_ERR_FILE_NAME_NOT_EXITS:u32 = 50003;
pub const EXT_AGREEMENT_ERR_CODE:u32 = 50004;
pub const EXT_LOCK_ERR_CODE:u32 = 50005;
pub const EXT_ERR_WRITE_FILE_FAILED:u32 = 50006;
pub const EXT_ERR_NOT_KNOW:u32 = 50007;
pub const EXT_ERR_SYNC_DATA:u32 = 50008;
pub const EXT_ERR_ALREADY_CREATED:u32 = 50009;
pub const EXT_ERR_NO_ACCESS_PERMISSION:u32 = 50010;