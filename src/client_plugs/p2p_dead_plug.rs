use std::{sync::Arc};
use rcmd_suit::{handler::SubHandle, plug::Plug};
use std::collections::{HashMap, VecDeque};
use tokio::sync::Mutex;
use rcmd_suit::ab_client::AbClient;
use rcmd_suit::config_build::Config;
use async_trait::async_trait;
use rcmd_suit::client_handler;
use rcmd_suit::utils::stream_parser::StreamParse;

use crate::extc::*;

use super::p2p_plugs::PlugData;
use rcmd_suit::utils::stream_parser::Stream;
use std::net::SocketAddr;


pub struct P2POnDeadPlugClientSer{
    plug_data: Arc<Mutex<PlugData>>,
    curr_sender: Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>,
    relay_map: Arc<Mutex<HashMap<SocketAddr,usize>>>
}

impl P2POnDeadPlugClientSer{
    pub fn new(
        plug_data: Arc<Mutex<PlugData>>,
        curr_sender: Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>,
        relay_map: Arc<Mutex<HashMap<SocketAddr,usize>>>
    )-> P2POnDeadPlugClientSer
    {
        P2POnDeadPlugClientSer{
            plug_data,
            curr_sender,
            relay_map
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
                drop(sender);

                if let Some(addr) = _link.relay()
                {
                    let mut relay = self.relay_map.lock().await;
                    let res = relay.remove(&addr);
                    println!("Rm link relay {:?}",res);
                }
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
    type ABClient = AbClient;

    type Id = usize;

    async fn handle(&self, data:&[u8], len:u32, ext:u32, clients:&Arc<Mutex<HashMap<Self::Id,Box<Self::ABClient>>>>, id:Self::Id) -> Option<(Vec<u8>,u32)>
    where Self::Id :Copy {
        let key = Arc::new(String::from_utf8_lossy(data).to_string());
        let mut d = self.plug_data.lock().await;

        if let Some(cpid) = d.get_cpid_verify_code(key)
        {
            if d.client_connected(cpid, id)
            {
                
            }else{
                let mut res = EXT_ERR_P2P_BAD_REQUEST.to_be_bytes().to_vec();
                res.extend_from_slice(&data[..]);
                return Some((res,EXT_P2P_CLIENT_VERIFY_SC));
            }
        }else{
            let mut res = EXT_ERR_P2P_BAD_VERIFY_CODE.to_be_bytes().to_vec();
            res.extend_from_slice(&data[..]);
            return Some((res,EXT_P2P_CLIENT_VERIFY_SC));
        }
        let mut res = 0u32.to_be_bytes().to_vec();
        res.extend_from_slice(&data[..]);
        Some((res,EXT_P2P_CLIENT_VERIFY_SC))
    }

    fn interested(&self, ext:u32) ->bool {
        ext == EXT_P2P_CLIENT_VERIFY_CS
    }
    
}

pub struct P2PVerifyClientHandler{
    plug_data: Arc<Mutex<PlugData>>,
    curr_sender: Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>
}

impl P2PVerifyClientHandler{
    pub fn new(
        plug_data: Arc<Mutex<PlugData>>,
        curr_sender: Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>
    )-> P2PVerifyClientHandler
    {
        P2PVerifyClientHandler{
            plug_data,
            curr_sender
        }
    }
}

#[async_trait]
impl client_handler::SubHandle for P2PVerifyClientHandler{
    async fn handle(&self, data: &[u8], len: u32, ext: u32) -> Option<(Vec<u8>, u32)> {
        let mut stream = Stream::new(data);
        if let Some(v) = u32::stream_parse(&mut stream){
            if v != 0
            {
                println!("p2p最终验证返回失败 code = {}",v);
                return None;
            }
        }else{
            return None;   
        };
        let code = Arc::new(String::from_utf8_lossy( stream.get_rest()).to_string());
        let mut d = self.plug_data.lock().await;
        if let Some(cpid) = d.get_cpid_verify_code(code)
        {
            if d.ser_connect_succ_repo(cpid)
            {
                println!("p2p链接成功ヾ(≧▽≦*)o");
                let mut sender = self.curr_sender.lock().await;
                sender.push_back((cpid.to_be_bytes().to_vec(),EXT_P2P_CONNECT_SUCCESS_CS));
            }
        }
        None
    }

    fn interested(&self, ext: u32) -> bool {
        ext == EXT_P2P_CLIENT_VERIFY_SC
    }
}