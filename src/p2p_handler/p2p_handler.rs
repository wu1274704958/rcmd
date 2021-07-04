use rcmd_suit::handler::SubHandle;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::Mutex;
use async_trait::async_trait;
use rcmd_suit::ab_client::AbClient;
use std::net::SocketAddr;
use std::time::SystemTime;
use rcmd_suit::tools;

enum LinkState{
    WaitResponse,
    Agreed,
    TryConnectAToB(usize,u8,SocketAddr,SystemTime,u8),
    TryConnectBToA(usize,u8,SocketAddr,SystemTime,u8),
    Failed,
    Success(usize)
}

struct LinkData{
    a:usize,
    b:usize,
    state:LinkState,
    key:String,
    verify_code:String
}

pub struct P2PLinkData
{

}

pub struct P2PHandlerSer{

}

#[async_trait]
impl SubHandle for P2PHandlerSer{
    type ABClient = AbClient;
    type Id = usize;

    async fn handle(&self, data: &[u8], len: u32, ext: u32, clients: &Arc<Mutex<HashMap<Self::Id, Box<Self::ABClient>>>>, id: Self::Id) -> Option<(Vec<u8>, u32)> where Self::Id: Copy {

    }
    fn interested(&self,ext:u32)->bool{
        ext == EXT_REQ_LINK_P2P_CS
    }

}

impl LinkData {
    pub fn gen_key(a:usize,b:usize) -> String
    {
        return if a > b {
            format!("{}{}", a, b)
        } else {
            format!("{}{}", b, a)
        }
    }
    pub fn gen_verify_code(key:&String) -> String
    {
         format!("{}@{}", tools::uuid(),*key)
    }
    pub fn new(a:usize,b:usize,key:String) -> LinkData
    {
        let verify_code = Self::gen_verify_code(&key);
        LinkData{
            a,b,
            state:LinkState::WaitResponse,
            key,
            verify_code
        }
    }
}