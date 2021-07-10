use rcmd_suit::{client_plug::client_plug::ClientPlug, utils::stream_parser::{Stream,StreamParse}};
use std::{collections::{HashMap, VecDeque}, net::{Ipv4Addr}, sync::{Arc,Weak}};
use tokio::net::UdpSocket;
use rcmd_suit::utils::udp_sender::USErr;
use async_trait::async_trait;
use std::net::SocketAddr;
use tokio::sync::Mutex;

use crate::extc::*;

pub enum P2PErr {
    LinkExist,
    NotFindAnyLocalAddr,
    NotReady,
    BadState
}

enum LinkState{
    WaitAccept,
    Accepted,
    ReadyToTryConnect,
    TryConnect(SocketAddr),
    Stage1Success(bool),
    Stage2Success(bool),
    Disconnected,
    ConnectFailed,
}

struct LinkData
{
    cp: usize,
    state: LinkState,
    verify_code :Option<Weak<String>>,
    connected_addr: Option<SocketAddr>
}

struct PlugData{
    socket:Option<Arc<UdpSocket>>,
    port: u16,
    link_map: HashMap<usize,LinkData>,
    cpid_map: HashMap<Arc<String>,usize>,

}

impl PlugData {
    pub fn new() -> PlugData
    {
        PlugData{
            socket : None,
            port : 0,
            link_map: HashMap::new(),
            cpid_map: HashMap::new(),
        }
    }
}

pub struct P2PPlug
{
    data : Arc<Mutex<PlugData>>,
    curr_sender: Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>
}

impl P2PPlug
{
    pub fn new(curr_sender: Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>)-> P2PPlug
    {
        P2PPlug{
            data : Arc::new(Mutex::new(PlugData::new())),
            curr_sender 
        }
    }

    fn get_local_ip() -> Vec<Ipv4Addr>
    {
        let mut res = Vec::new();
        for iface in get_if_addrs::get_if_addrs().unwrap() {
            if !iface.addr.is_loopback() {
                if let std::net::IpAddr::V4(ip) = iface.addr.ip(){
                    res.push(ip);
                }
            }
        }
        res
    }

    pub async fn req_link(&self,cpid:usize) -> Result<(),P2PErr>
    {
        {
            let d = self.data.lock().await;
            if d.link_map.contains_key(&cpid)
            {
                return Err(P2PErr::LinkExist);
            }
        }

        let port = {
            let d = self.data.lock().await;
            d.port
        };
        if port == 0{
            return Err(P2PErr::NotReady);
        }
        let mut d = Vec::new();
        d.extend_from_slice(cpid.to_be_bytes().as_ref());
        let ips = Self::get_local_ip();
        if ips.is_empty(){
            return Err(P2PErr::NotFindAnyLocalAddr);
        }
        d.push(ips.len() as u8);
        d.extend_from_slice(port.to_be_bytes().as_ref());

        for i in ips.into_iter(){
            d.extend_from_slice(i.octets().as_ref());
        }

        self.add_link(LinkState::WaitAccept, cpid).await;

        self.send_data(d, EXT_REQ_HELP_LINK_P2P_CS).await;
    
        Ok(())
    }

    pub async fn accept_p2p(&self,cpid:usize,accept:bool) -> Result<(),P2PErr>
    {
        let port = {
            let d = self.data.lock().await;
            d.port
        };
        if port == 0{
            return Err(P2PErr::NotReady);
        }
        let mut d = Vec::new();
        d.extend_from_slice(cpid.to_be_bytes().as_ref());
        d.push(if accept {1}else{0});
        let ips = Self::get_local_ip();
        if ips.is_empty(){
            return Err(P2PErr::NotFindAnyLocalAddr);
        }
        d.push(ips.len() as u8);
        d.extend_from_slice(port.to_be_bytes().as_ref());

        for i in ips.into_iter(){
            d.extend_from_slice(i.octets().as_ref());
        }

        if !accept{
            self.rm_link(cpid).await;
        }else{
            self.set_state(cpid, LinkState::Accepted).await;
        }

        self.send_data(d, EXT_ACCEPT_LINK_P2P_CS).await;
    
        Ok(())
    }

    async fn send_data(&self,d:Vec<u8>,ext:u32)
    {
        let mut se = self.curr_sender.lock().await;
        se.push_back((d,ext));
    }

    async fn rm_link(&self,cpid:usize) -> Option<LinkData>
    {
        let mut d = self.data.lock().await;
        if let Some(link) = d.link_map.remove(&cpid)
        {
            if let Some(ref s) = link.verify_code{
                if let Some(ref s) = s.upgrade()
                {
                    d.cpid_map.remove(s);
                }
            }
            Some(link)
        }else{
            None
        }
    }

    async fn add_link(&self,st:LinkState,cpid:usize) -> bool
    {
        let mut d = self.data.lock().await;
        if d.link_map.contains_key(&cpid) {
            return false;
        }
        d.link_map.insert(cpid, LinkData{
            cp :cpid,
            state : st,
            verify_code : None,
            connected_addr:None
        });
        true
    }

    async fn set_verify_code(&self,cpid:usize,code:String) -> bool
    {
        let mut d = self.data.lock().await;
        let c = Arc::new(code);
        if d.cpid_map.contains_key(&c)
        {
            return false;
        }
        if let Some(data) = d.link_map.get_mut(&cpid) {
            let weak = Arc::downgrade(&c);
            data.verify_code = Some(weak);
            drop(data);
            d.cpid_map.insert(c, cpid);
            true
        }else{
            false
        }
    }

    async fn get_cpid_verify_code(&self,code:String) -> Option<usize>
    {
        let c = Arc::new(code);
        let d = self.data.lock().await;
        if let Some(cpid) = d.cpid_map.get(&c)
        {
            Some(*cpid)
        }else{
            None
        }
    }

    async fn set_state(&self,cpid:usize,st:LinkState) -> bool
    {
        let mut d = self.data.lock().await;
        if let Some(d) = d.link_map.get_mut(&cpid)
        {
            d.state = st;
            true
        }else{
            false
        }
    }

}

#[async_trait]
impl ClientPlug for P2PPlug
{
    type SockTy = UdpSocket;
    type ErrTy = USErr;

    async fn on_init(&self) {

    }

    async fn on_create_socket(&self, sock: Arc<Self::SockTy>) {
        let mut d = self.data.lock().await;
        d.socket = Some(sock.clone());
    }

    async fn on_get_local_addr(&self, addr: SocketAddr) {
        dbg!(addr);
        let mut d = self.data.lock().await;
        d.port = addr.port();
    }

    async fn on_get_err(&self, _err: Self::ErrTy) where Self::ErrTy: Clone {

    }

    async fn on_lauch_recv_worker(&self) {

    }

    async fn on_stop(&self) {

    }

    async fn on_recv_oth_msg(&self, addr: SocketAddr, data: &[u8]) {
        println!("recv other msg from {:?}\n{:?}",&addr,data);
    }

    async fn on_lauch_loop(&self) {

    }
    #[allow(non_snake_case)]
    async fn handle(&self, _msg:rcmd_suit::agreement::Message<'_>) {
        match _msg.ext{
            EXT_REQ_LINK_P2P_SC => {
                let mut stream = Stream::new(_msg.msg);
                if let Some(cpid) = usize::stream_parse(&mut stream)
                {
                    self.add_link(LinkState::WaitAccept, cpid).await;
                    //先自动同意
                    self.accept_p2p(cpid, true).await;
                }
            }
            EXT_REQ_LINK_P2P_REJECTED_SC => {
                let mut stream = Stream::new(_msg.msg);
                if let Some(cpid) = usize::stream_parse(&mut stream)
                {
                    self.rm_link(cpid).await;
                }
            }
            
            _=>{}
        }
    }

}