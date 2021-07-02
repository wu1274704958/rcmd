use rcmd_suit::handler::SubHandle;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::Mutex;
use async_trait::async_trait;
use rcmd_suit::ab_client::AbClient;
use std::net::SocketAddr;
use std::time::SystemTime;

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
    key:String
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
    pub fn new(a:usize,b:usize,) -> LinkData
    {
        let key Self::gen_key()
        LinkData{
            a,b,
        }
    }
}