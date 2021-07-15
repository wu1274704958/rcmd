use std::{sync::Arc};
use rcmd_suit::{handler::SubHandle, plug::Plug};
use std::collections::{HashMap, VecDeque};
use tokio::sync::Mutex;
use rcmd_suit::ab_client::AbClient;
use rcmd_suit::config_build::Config;
use async_trait::async_trait;

use crate::extc::{EXT_P2P_CLIENT_DISCONNECT_CS, EXT_P2P_CLIENT_VERIFY_CS, EXT_P2P_CONNECT_SUCCESS_CS};

use super::p2p_plugs::PlugData;


pub struct P2POnDeadPlugClientSer{
    plug_data: Arc<Mutex<PlugData>>,
    curr_sender: Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>
}

impl P2POnDeadPlugClientSer{
    pub fn new(
        plug_data: Arc<Mutex<PlugData>>,
        curr_sender: Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>
    )-> P2POnDeadPlugClientSer
    {
        P2POnDeadPlugClientSer{
            plug_data,
            curr_sender
        }
    }
}

#[async_trait]
impl Plug for P2POnDeadPlugClientSer {
    type ABClient = AbClient;
    type Id = usize;
    type Config = Config;

    async fn run(&self, id: Self::Id, _clients: &Arc<Mutex<HashMap<Self::Id, Box<Self::ABClient>>>>, _config: Arc<Self::Config>) where Self::Id: Copy {
        let mut d = self.plug_data.lock().await;
        if let Some(cpid) = d.ser_map.remove(&id)
        {
            if let Some(_link) = d.rm_link(cpid)
            {
                let mut sender = self.curr_sender.lock().await;
                sender.push_back((cpid.to_be_bytes().to_vec(),EXT_P2P_CLIENT_DISCONNECT_CS));
            }
        }
    }
}

pub struct P2PVerifyHandler{
    plug_data: Arc<Mutex<PlugData>>,
    curr_sender: Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>
}

impl P2PVerifyHandler{
    pub fn new(
        plug_data: Arc<Mutex<PlugData>>,
        curr_sender: Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>
    )-> P2PVerifyHandler
    {
        P2PVerifyHandler{
            plug_data,
            curr_sender
        }
    }
}

#[async_trait]
impl SubHandle for P2PVerifyHandler{

    
    async fn handle(&self, data:&[u8], len:u32, ext:u32, clients:&Arc<Mutex<HashMap<Self::Id,Box<Self::ABClient>>>>, id:Self::Id) -> Option<(Vec<u8>,u32)>
    where Self::Id :Copy {
        let key = Arc::new(String::from_utf8_lossy(data).to_string());
        let mut d = self.plug_data.lock().await;

        if let Some(cpid) = d.get_cpid_verify_code(key)
        {
            if d.client_connected(cpid, id)
            {
                let mut sender = self.curr_sender.lock().await;
                sender.push_back((cpid.to_be_bytes().to_vec(),EXT_P2P_CONNECT_SUCCESS_CS));
            }
        }

        None
    }

    fn interested(&self, ext:u32) ->bool {
        ext == EXT_P2P_CLIENT_VERIFY_CS
    }

    type ABClient = AbClient;

    type Id = usize;
    
}