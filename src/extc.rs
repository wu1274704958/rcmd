#![allow(dead_code)]
// ext 9        heartbeat
// ext 0        default
// ext 10 .. 20  asymmetric cryptographic
// ext 20 .. 40  asymmetric cryptographic err
// ext 40 41      upload file

pub const EXT_UPLOAD_FILE:u32 = 40;
pub const EXT_UPLOAD_FILE_ELF:u32 = 41;
pub const EXT_UPLOAD_FILE_CREATE:u32 = 42;
pub const EXT_LOGIN:u32 = 43;
pub const EXT_LOGOUT:u32 = 44;
pub const EXT_GET_USERS:u32 = 45;
pub const EXT_REGISTER:u32 = 46;
pub const EXT_SEND_MSG:u32 = 47;
pub const EXT_SEND_BROADCAST:u32 = 48;
pub const EXT_RECV_MSG:u32 = 49;
pub const EXT_EXEC_CMD:u32 = 50;
pub const EXT_RUN_CMD:u32 = 51;
pub const EXT_SEND_FILE:u32 = 52;
pub const EXT_SEND_FILE_ELF:u32 = 53;
pub const EXT_SEND_FILE_CREATE:u32 = 54;
pub const EXT_SAVE_FILE:u32 = 55;
pub const EXT_SAVE_FILE_ELF:u32 = 56;
pub const EXT_SAVE_FILE_CREATE:u32 = 57;
pub const EXT_SAVE_FILE_RET:u32 = 58;
pub const EXT_SAVE_FILE_ELF_RET:u32 = 59;
pub const EXT_SAVE_FILE_CREATE_RET:u32 = 60;
pub const EXT_PULL_FILE_S:u32 = 61;
pub const EXT_PULL_FILE_C:u32 = 62;

//P2P 相关
pub const EXT_REQ_HELP_LINK_P2P_CS:u32 = 63;
pub const EXT_REQ_HELP_LINK_P2P_SC:u32 = 64;
pub const EXT_REQ_LINK_P2P_SC:u32 = 65;
pub const EXT_REQ_LINK_P2P_CS:u32 = 66;
pub const EXT_REQ_LINK_P2P_REJECTED_SC:u32 = 67;
pub const EXT_P2P_TRY_CONNECT_SC:u32 = 68;
pub const EXT_P2P_CONNECT_SUCCESS_STAGE1_CS:u32 = 69;
pub const EXT_P2P_CONNECT_SUCCESS_CS:u32 = 70;
pub const EXT_P2P_WAIT_CONNECT_SC:u32 = 71;
pub const EXT_P2P_SYNC_VERIFY_CODE_SC:u32 = 72;
pub const EXT_P2P_CONNECT_SUCCESS_STAGE1_SC:u32 = 73;
pub const EXT_P2P_CONNECT_SUCCESS_SC:u32 = 74;


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
pub const EXT_ERR_PARSE_ARGS:u32 = 50011;
pub const EXT_ERR_WRONG_PASSWORD:u32 = 50012;
pub const EXT_ERR_ALREADY_LOGIN:u32 = 50013;
pub const EXT_ERR_NOT_FOUND_ACC:u32 = 50014;
pub const EXT_ERR_NOT_LOGIN:u32 = 50015;
pub const EXT_ERR_PERMISSION_DENIED:u32 = 50016;
pub const EXT_ERR_BAD_ACCOUNT:u32 = 50017;
pub const EXT_ERR_BAD_USERNAME:u32 = 50018;
pub const EXT_ERR_BAD_PASSWORD:u32 = 50019;
pub const EXT_ERR_ACC_REGISTERED:u32 = 50020;
pub const EXT_ERR_NOT_FOUND_LID:u32 = 50021;
pub const EXT_ERR_BAD_TARGET:u32 = 50022;
pub const EXT_ERR_EXEC_CMD_NOT_KNOW:u32 = 50023;
pub const EXT_ERR_EXEC_CMD_RET_ERR:u32 = 50024;
pub const EXT_ERR_SAVE_FILE_RET_EXT:u32 = 50025;
pub const EXT_ERR_PULL_FILE_RET_EXT:u32 = 50026;
pub const EXT_ERR_BAD_FILE_PATH:u32 = 50027;
pub const EXT_ERR_OPEN_FILE:u32 = 50028;

//p2p 相关错误码
pub const EXT_ERR_LINK_DATA_ALREADY_EXIST:u32 = 50029;
pub const EXT_ERR_NOT_FOUND_LINK_DATA:u32 = 50030;
pub const EXT_ERR_ALREADY_ACCEPT:u32 = 50031;
pub const EXT_ERR_P2P_CP_OFFLINE:u32 = 50032;
pub const EXT_ERR_P2P_LINK_FAILED:u32 = 50033;
pub const EXT_ERR_P2P_BAD_REQUEST:u32 = 50034;