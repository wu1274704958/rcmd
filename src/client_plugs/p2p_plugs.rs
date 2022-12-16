use async_std::channel::{Receiver, Sender, unbounded};
use rcmd_suit::{ab_client::AbClient, agreement::DefParser, client_plug::client_plug::ClientPlug, config_build::ConfigBuilder, handler::{DefHandler, TestHandler}, plug::DefPlugMgr, plugs::heart_beat::HeartBeat, servers::udp_server::{UdpServer}, utils::stream_parser::{Stream, StreamParse}, client_handler};
use std::{cell::Cell, collections::{HashMap, VecDeque}, net::{Ipv4Addr}, sync::{Arc,Weak}, usize};
use tokio::{net::UdpSocket, runtime::{self, Runtime}, sync::MutexGuard};
use rcmd_suit::utils::udp_sender::{USErr, DefUdpSender, UdpSender};
use async_trait::async_trait;
use std::net::{SocketAddr, IpAddr};
use tokio::sync::Mutex;
use num_enum::TryFromPrimitive;
use crate::{comm, handlers};
use crate::extc::*;
use std::mem::size_of;
use std::convert::TryFrom;
use rcmd_suit::handler::Handle;
use rcmd_suit::plug::PlugMgr;

use super::p2p_dead_plug::{P2POnDeadPlugClientSer, P2PVerifyHandler};
use ahash::{RandomState, CallHasher};
use std::panic::resume_unwind;
use rcmd_suit::clients::udp_client::{UdpClient, IUdpClient, UdpClientErr};
use std::hash::BuildHasher;
use crate::client_plugs::p2p_dead_plug::P2PVerifyClientHandler;
use std::time::Duration;
use tokio::time::sleep;
use crate::client_plugs::attched_udp_sender::AttchedUdpSender;
use crate::client_plugs::udp_server_channel::run_udp_server_with_channel;
use crate::client_plugs::p2p_plugs::LocalEntity::Client;
use crate::client_plugs::p2p_event::P2PEvent;
use crate::p2p_client_handler::upload_file::UploadHandler;
use rcmd_suit::ab_client::{ABClient, State};
use local_ip_address::list_afinet_netifas;

#[repr(u16)]
#[derive(TryFromPrimitive)]
#[derive(Copy, Clone,Debug)]
#[allow(non_camel_case_types)]
enum Ext {
    Hello1 = 1,
    Hello2 = 2,
    Hello3 = 3,
    Hello4 = 4
}

impl Into<u16> for Ext{
    fn into(self) -> u16
    {
        self as u16
    }
}
#[allow(dead_code)]
#[derive(Debug)]
pub enum P2PErr {
    LinkExist,
    NotFindAnyLocalAddr,
    NotReady,
    BadState,
    NoEntity,
    NoLink,
    NotKnow,
    NotAttachedClient,
    Wrap((i32,String)),
    UdpClientErr(UdpClientErr)
}

impl From<std::io::Error> for P2PErr
{
    fn from(e: std::io::Error) -> Self {
        let code = match e.raw_os_error() {
            None => {-99999}
            Some(v) => {v}
        };
        let str = format!("{}",e);
        P2PErr::Wrap((code,str))
    }
}

impl From<UdpClientErr> for P2PErr{
    fn from(e: UdpClientErr) -> Self {
        P2PErr::UdpClientErr(e)
    }
}
#[allow(dead_code)]
#[derive(Eq, PartialEq,Debug)]
enum LinkState{
    WaitAccept,
    Accepted,
    TryConnect(SocketAddr),
    Stage1Success(bool),
    Stage2Success(bool),
    Disconnected,
    ConnectFailed,
    WaitingHello1,
    WaitingHello2,
    WaitingHello3,
    WaitingHello4,
    PrepareRealLink
}
#[derive(Clone, Copy)]
pub enum LocalEntity {
    Ser(usize),
    Client(u64),
    None
}

pub struct LinkData
{
    cp: usize,
    state: LinkState,
    verify_code :Option<Weak<String>>,
    connected_addr: Option<SocketAddr>,
    local_entity: LocalEntity,
    relay: Option<SocketAddr>
}

impl LinkData {
    fn get_verify_code(&self) -> Option<Arc<String>>
    {
        if let Some(ref c) = self.verify_code
        {
            return c.upgrade();
        }
        None
    }

    pub fn relay(&self) -> Option<SocketAddr> {
        self.relay
    }
}

pub struct PlugData{
    socket:Option<Arc<UdpSocket>>,
    port: u16,
    link_map: HashMap<usize,LinkData>,
    cpid_map: HashMap<Arc<String>,usize>,
    pub ser_map: HashMap<usize,usize>,
}

impl PlugData {
    pub fn new() -> PlugData
    {
        PlugData{
            socket : None,
            port : 0,
            link_map: HashMap::new(),
            cpid_map: HashMap::new(),
            ser_map: HashMap::new(),
        }
    }

    pub fn rm_link(&mut self,cpid:usize) -> Option<LinkData>
    {
        if let Some(link) = self.link_map.remove(&cpid)
        {
            if let Some(ref s) = link.verify_code{
                if let Some(ref s) = s.upgrade()
                {
                    self.cpid_map.remove(s);
                }
            }
            Some(link)
        }else{
            None
        }
    }

    pub fn get_cpid_verify_code(&self,code:Arc<String>) -> Option<usize>
    {
        if let Some(cpid) = self.cpid_map.get(&code)
        {
            Some(*cpid)
        }else{
            None
        }
    }

    pub fn client_connected(&mut self,cp:usize,cid:usize) -> bool
    {
        if self.ser_map.contains_key(&cid)
        {
            return false;
        }
        if let Some(link) = self.link_map.get_mut(&cp)
        {
            if let LocalEntity::None = link.local_entity{
                link.state = LinkState::Stage2Success(true);
                link.local_entity = LocalEntity::Ser(cid);
                self.ser_map.insert(cid, cp);
            }else{
                return false;
            }
        }else{
            return false;
        }
        true
    }

    pub fn ser_connect_succ_repo(&mut self,cp:usize) -> bool
    {
        if let Some(link) = self.link_map.get_mut(&cp)
        {
            link.state = LinkState::Stage2Success(false);
            true
        }else{
            false
        }
    }
}

pub struct P2PPlug
{
    data : Arc<Mutex<PlugData>>,
    curr_sender: Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>,
    runtime: Arc<Mutex<Option<runtime::Runtime>>>,
    ser_sender: Mutex<Option<Sender<(SocketAddr,Vec<u8>)>>>,
    ser_clients: Arc<Mutex<HashMap<usize,Box<AbClient>>>>,
    hash_builder: RandomState,
    cli_map: Arc<Mutex<HashMap<u64,(usize,Option<Arc<dyn IUdpClient>>)>>>,
    cli_sender_map : Arc<Mutex<HashMap<u64,Sender<Vec<u8>>>>>,
    relay_map: Arc<Mutex<HashMap<SocketAddr,usize>>>,
    event:Option<Arc<dyn P2PEvent>>
}

impl P2PPlug
{
    pub fn new(curr_sender: Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>,
               event:Option<Arc<dyn P2PEvent>>)-> P2PPlug
    {

        P2PPlug{
            data : Arc::new(Mutex::new(PlugData::new())),
            curr_sender,
            runtime: Arc::new(Mutex::new(None)),
            ser_sender :Mutex::new(None),
            ser_clients : Arc::new(Mutex::new(HashMap::new())),
            hash_builder: RandomState::new(),
            cli_map : Arc::new(Mutex::new(HashMap::new())),
            cli_sender_map : Arc::new(Mutex::new(HashMap::new())),
            relay_map : Arc::new(Mutex::new(HashMap::new())),
            event
        }
    }

    pub async fn has_entity(&self,cpid:usize) -> bool
    {
        let d = self.data.lock().await;
        if let Some(link) = d.link_map.get(&cpid)
        {
            return if let LocalEntity::None = link.local_entity
            {
                false
            }else {
                true
            };
        }
        false
    }

    pub async fn send_msg(&self,cpid:usize,data:Vec<u8>,ext:u32) -> Result<(),P2PErr>
    {
        let d = self.data.lock().await;
        if let Some(link) = d.link_map.get(&cpid)
        {
            return match link.local_entity {
                LocalEntity::Ser(id) => {
                    let mut scs =  self.ser_clients.lock().await;
                    if let Some(c) = scs.get_mut(&id)
                    {
                        c.push_msg(data,ext);
                    }else{
                        return Err(P2PErr::NotKnow);
                    }
                    Ok(())
                }
                Client(id) => {
                    let mut cs = self.cli_map.lock().await;
                    if let Some(c) = cs.get_mut(&id)
                    {
                        if let Some(cli) = c.1.clone(){
                            cli.send(data,ext).await;
                        }else{
                            return Err(P2PErr::NotAttachedClient);
                        }
                    }else{
                        return Err(P2PErr::NotKnow);
                    }
                    Ok(())
                }
                LocalEntity::None => { return Err(P2PErr::NoEntity); }
            }
        }
        Err(P2PErr::NoLink)
    }

    pub async fn send_msg_ex(&self,cpid:usize,ds:Vec<(Vec<u8>,u32)>) -> Result<(),P2PErr>
    {
        let d = self.data.lock().await;
        if let Some(link) = d.link_map.get(&cpid)
        {
            return match link.local_entity {
                LocalEntity::Ser(id) => {
                    let mut scs =  self.ser_clients.lock().await;
                    if let Some(c) = scs.get_mut(&id)
                    {
                        for (data,ext) in ds.into_iter() {
                            c.push_msg(data, ext);
                        }
                    }else{
                        return Err(P2PErr::NotKnow);
                    }
                    Ok(())
                }
                Client(id) => {
                    let mut cs = self.cli_map.lock().await;
                    if let Some(c) = cs.get_mut(&id)
                    {
                        if let Some(v) = c.1.clone(){
                            for data  in ds.into_iter() {
                                v.send(data.0, data.1).await;
                            }
                        }else{
                            return Err(P2PErr::NotAttachedClient);
                        }
                    }else{
                        return Err(P2PErr::NotKnow);
                    }
                    Ok(())
                }
                LocalEntity::None => { return Err(P2PErr::NoEntity); }
            }
        }
        Err(P2PErr::NoLink)
    }

    pub fn get_local_ip() -> Vec<Ipv4Addr>
    {
        let mut res = Vec::new();
        let network_interfaces = list_afinet_netifas().unwrap();
        for (name, ip) in network_interfaces.iter() {
            if !ip.is_loopback() {
                if let std::net::IpAddr::V4(ip) = ip{
                    res.push(ip.clone());
                }
            }
            println!("ip ---> {}:\t{:?}", name, ip);
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

    pub async fn req_close(&self,cpid:usize) -> Result<(),P2PErr>
    {
        let d = self.data.lock().await;
        if let Some(link) = d.link_map.get(&cpid)
        {
            return match link.local_entity {
                LocalEntity::Ser(id) => {
                    let mut scs =  self.ser_clients.lock().await;
                    if let Some(c) = scs.get_mut(&id)
                    {
                        c.set_state(State::ReqClose);
                    }else{
                        return Err(P2PErr::NotKnow);
                    }
                    Ok(())
                }
                Client(id) => {
                    let mut cs = self.cli_map.lock().await;
                    if let Some(c) = cs.get_mut(&id)
                    {
                        if let Some(cli) = c.1.clone(){
                            cli.close_session().await?;
                        }else{
                            return Err(P2PErr::NotAttachedClient);
                        }
                    }else{
                        return Err(P2PErr::NotKnow);
                    }
                    Ok(())
                }
                LocalEntity::None => { return Err(P2PErr::NoEntity); }
            }
        }
        Err(P2PErr::NoLink)
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
        d.rm_link(cpid)
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
            connected_addr:None,
            local_entity: LocalEntity::None,
            relay: None
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

    pub async fn get_cpid_verify_code(&self,code:String) -> Option<usize>
    {
        let c = Arc::new(code);
        let d = self.data.lock().await;
        d.get_cpid_verify_code(c)
    }

    async fn get_verify_code_cpid(&self,id:usize) -> Option<Arc<String>>
    {
        let d = self.data.lock().await;
        if let Some(d) = d.link_map.get(&id)
        {
            if let Some(ref c) = d.verify_code
            {
                return c.upgrade();
            }
        }
        None
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

    async fn prepare_runtime<F>(&self,f:F)
        where F:FnOnce(&Runtime)
    {
        let mut rt = self.runtime.lock().await;
        if let Some(ref r) = *rt
        {
            f(r);
        }else{
            let r = runtime::Builder::new_multi_thread()
                .worker_threads(8)
                .enable_time()
                .build()
                .unwrap();
            f(&r);
            *rt = Some(r);
        }
    }


    fn need_parse_cpid(ext:u32) -> bool
    {
        ext == EXT_REQ_LINK_P2P_SC ||
        ext == EXT_ERR_P2P_LINK_FAILED||
        ext == EXT_ERR_P2P_CP_OFFLINE ||
        ext == EXT_REQ_LINK_P2P_REJECTED_SC ||
        ext == EXT_P2P_SYNC_VERIFY_CODE_SC ||
        ext == EXT_P2P_WAIT_CONNECT_SC ||
        ext == EXT_P2P_TRY_CONNECT_SC ||
        ext == EXT_P2P_NOTIFY_RELAY_SC ||
        ext == EXT_ERR_P2P_WAIT_ACCEPT_TIMEOUT
    }

    fn parse_addr(s:&mut Stream) -> Option<SocketAddr>
    {
        let ip = if let Some(r) = s.next_range(4)
        {
            Ipv4Addr::new(r[0],r[1],r[2],r[3])
        }else{
            return None;
        };
        let port = if let Some(port) = u16::stream_parse(s)
        {
            port
        }else{return None;};
        Some(SocketAddr::new(IpAddr::V4(ip),port))
    }

    async fn get_socket(&self) -> Option<Arc<UdpSocket>>
    {
        let d = self.data.lock().await;
        if let Some(ref s) = d.socket
        {
            return Some(s.clone());
        }
        None
    }

    async fn send_udp_msg(&self,addr:SocketAddr,msg:&[u8],times:u8) -> Result<(),P2PErr>
    {
        {
            let relay = self.relay_map.lock().await;
            if let Some(cpid) = relay.get(&addr)
            {
                let mut d = Vec::with_capacity(msg.len() + size_of::<usize>());
                d.extend_from_slice(cpid.to_be_bytes().as_ref());
                d.extend_from_slice(msg);
                self.send_data(d,EXT_P2P_RELAY_MSG_CS).await;
                return Ok(());
            }
        }
        let d = self.data.lock().await;
        if let Some(ref s) = d.socket
        {
            for _ in 0..times{
                if let Err(e) = s.send_to(msg,addr).await
                {
                    return Err(P2PErr::from(e));
                }
            }
        }else{
            return Err(P2PErr::NotReady);
        }
        Ok(())
    }

    fn wrap(d:&[u8],ext:u16) -> Vec<u8>
    {
        let mut res = Vec::new();
        res.push(19);
        let l = (d.len() + Self::wrap_len()) as u32;
        res.extend_from_slice( l.to_be_bytes().as_ref() );
        res.extend_from_slice(ext.to_be_bytes().as_ref());
        res.extend_from_slice(d);
        res.push(20);
        res
    }

    const fn wrap_len() -> usize { size_of::<u8>() + size_of::<u8>() + size_of::<u32>() + size_of::<u16>() }

    fn get_waiting_st(ext:Ext,addr:SocketAddr) -> LinkState
    {
        match ext {
            Ext::Hello1 => LinkState::WaitingHello1,
            Ext::Hello2 => LinkState::TryConnect(addr),
            Ext::Hello3 => LinkState::WaitingHello3,
            Ext::Hello4 => LinkState::Stage1Success(false),
        }
    }

    fn get_next_st(ext:Ext) -> LinkState
    {
        match ext {
            Ext::Hello1 => LinkState::WaitingHello3,
            Ext::Hello2 => LinkState::Stage1Success(false),
            Ext::Hello3 => LinkState::Stage1Success(true),
            Ext::Hello4 => LinkState::PrepareRealLink,
        }
    }

    fn unwrap(d:&[u8]) -> Option<(&[u8],u16)>
    {
        let l = d.len();
        if l > 1 && d[0] == 19 && d[l - 1] == 20
        {
            let mut s = Stream::new(d);
            s.next();
            if let Some(len) = u32::stream_parse(&mut s)
            {
                if len == l as u32
                {
                    if let Some(ext) = u16::stream_parse(&mut s)
                    {
                        return if let Some(v) = s.next_range(l - Self::wrap_len())
                        {
                            Some((v,ext))
                        }else{
                            None
                        };
                    }else{
                        return None;
                    }
                }
            }
        }
        None
    }

    async fn add_client(&self,ck:u64,s:Sender<Vec<u8>>,cli:Option<Arc<dyn IUdpClient>>,cp:usize) -> bool
    {
        let mut cli_sender_map = self.cli_sender_map.lock().await;
        if cli_sender_map.contains_key(&ck)
        {
            return false;
        }
        cli_sender_map.insert(ck,s);
        drop(cli_sender_map);
        let mut cli_map = self.cli_map.lock().await;
        cli_map.insert(ck,(cp,cli));
        true
    }
    async fn attached_client(&self,ck:u64,cli:Arc<dyn IUdpClient>) -> bool
    {
        Self::attached_client_ex(self.cli_map.clone(),ck,cli).await
    }
    async fn attached_client_ex(cli_map_: Arc<Mutex<HashMap<u64,(usize,Option<Arc<dyn IUdpClient>>)>>>,
                                ck:u64,cli:Arc<dyn IUdpClient>) -> bool
    {
        let mut cli_map = cli_map_.lock().await;
        if let Some(v)  = cli_map.get_mut(&ck)
        {
            if (*v).1.is_some()
            {
                eprintln!("p2p attached_client too many times!!!");
                false
            }else{
                (*v).1 = Some(cli);
                true
            }
        }else {
            false
        }
    }
    async fn rm_client(&self,ck:u64) -> Option<usize>
    {
        let mut cli_sender_map = self.cli_sender_map.lock().await;
        if !cli_sender_map.contains_key(&ck)
        {
            return None;
        }
        cli_sender_map.remove(&ck);
        drop(cli_sender_map);
        let mut cli_map = self.cli_map.lock().await;
        if let Some(v) = cli_map.remove(&ck)
        {
            return Some(v.0);
        }
        None
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
        {
            let mut ser_sender = self.ser_sender.lock().await;
            *ser_sender = None;
        }
        {
            let mut cli_sender_map = self.cli_sender_map.lock().await;
            cli_sender_map.clear();
        }
        {
            let mut runtime = self.runtime.lock().await;
            if let Some(r) = runtime.take(){
                println!("Drop runtime");
                sleep(Duration::from_millis(1000)).await;
                r.shutdown_background();
            }
        }

    }

    async fn on_recv_oth_msg(&self, addr: SocketAddr, data: &[u8]) {
        //println!("recv other msg from {:?}\n{:?}",&addr,data);
        if let Some((msg,ext)) = Self::unwrap(data)
        {

            let e =  Ext::try_from(ext).unwrap();
            match e {
                Ext::Hello1 |
                Ext::Hello2 |
                Ext::Hello3 |
                Ext::Hello4
                => {
                    let code = String::from_utf8_lossy(msg).to_string();
                    if let Some(cpid) = self.get_cpid_verify_code(code).await{
                        let mut send_hi = false;
                        let mut d = self.data.lock().await;
                        if let Some(link) = d.link_map.get_mut(&cpid)
                        {
                            if link.state == Self::get_waiting_st(e,addr){
                                println!("get ext {:?} from {}",ext,addr);
                                send_hi = if ext == 4 { false } else{ true };
                                link.state = Self::get_next_st(e);

                                match e {
                                    Ext::Hello2 => {
                                        link.connected_addr = Some(addr);
                                    }
                                    Ext::Hello3 => {
                                        link.connected_addr = Some(addr);
                                        //准备p2p服务器
                                        println!("准备p2p服务器");
                                         let mut ser_sender = self.ser_sender.lock().await;
                                         if ser_sender.is_none(){
                                             let (s,rx) = unbounded::<(SocketAddr,Vec<u8>)>();
                                             *ser_sender = Some(s);
                                             let cls = self.ser_clients.clone();
                                             let plug_data_cp = self.data.clone();
                                             let curr_sender_cp = self.curr_sender.clone();
                                             let sock = d.socket.as_ref().unwrap().clone();
                                             let relay_map = self.relay_map.clone();
                                             self.prepare_runtime(|runtime|{
                                                 runtime.spawn(lauch_p2p_ser(
                                                     rx, sock, cls,
                                                     plug_data_cp, curr_sender_cp,relay_map));
                                             }).await;
                                         }
                                    }
                                    Ext::Hello4 => {
                                        self.send_data(link.cp.to_be_bytes().to_vec(),EXT_P2P_CONNECT_SUCCESS_STAGE1_CS).await;
                                        //准备p2p client
                                        println!("准备p2p client");
                                        let cpid = link.cp;
                                        let is_relay = link.relay.is_some();
                                        let addr_key = CallHasher::get_hash(&addr,self.hash_builder.build_hasher());
                                        let (s,rx) = unbounded::<Vec<u8>>();
                                        let msg_queue = Arc::new(Mutex::new(VecDeque::new()));
                                        let plug_data_cp = self.data.clone();
                                        let curr_sender_cp = self.curr_sender.clone();
                                        let verify_code = link.get_verify_code().unwrap();
                                        let cli_sender_map = self.cli_sender_map.clone();
                                        let cli_map = self.cli_map.clone();
                                        link.local_entity = LocalEntity::Client(addr_key);
                                        self.add_client(addr_key,s,None,link.cp).await;
                                        let sock = d.socket.as_ref().unwrap().clone();
                                        let relay_map = self.relay_map.clone();


                                        self.prepare_runtime(|runtime|{
                                            if is_relay {
                                                runtime.spawn(lauch_p2p_client_relay(
                                                    addr, rx, msg_queue, addr_key,cpid, plug_data_cp,
                                                    curr_sender_cp, verify_code, cli_sender_map, cli_map, relay_map));
                                            }
                                            else {
                                                runtime.spawn(lauch_p2p_client(
                                                    addr, rx, sock, msg_queue, addr_key, plug_data_cp,
                                                    curr_sender_cp, verify_code, cli_sender_map, cli_map, relay_map));
                                            }
                                        }).await;
                                    }
                                    _ => {}
                                }
                            }else{
                                //println!("not eq {:?} {:?}",link.state,Self::get_waiting_st(e,addr));
                            }
                        }
                        drop(d);
                        if send_hi{
                            let d = Self::wrap(msg,ext + 1);
                            if let Err(e) = self.send_udp_msg(addr,d.as_slice(),18).await
                            {
                                println!("send udp msg {} failed {:?}",ext + 1,e);
                            }
                        }
                    }
                }
            }
        }else{
            let addr_key = CallHasher::get_hash(&addr,self.hash_builder.build_hasher());

            let mut cli_sender = self.cli_sender_map.lock().await;

            if let Some(sender) = cli_sender.get_mut(&addr_key)
            {
                sender.send(data.to_vec()).await;
                return;
            }
            drop(cli_sender);

            let mut ser_sender = self.ser_sender.lock().await;
            if let Some(ref mut sender) = *ser_sender{
                sender.send((addr,data.to_vec())).await;
            }
        }
    }

    async fn on_lauch_loop(&self) {

    }
    #[allow(non_snake_case)]
    async fn handle(&self, _msg:rcmd_suit::agreement::Message<'_>) {
        if match _msg.ext {
            EXT_REQ_LINK_P2P_SC|
            EXT_ERR_P2P_LINK_FAILED|
            EXT_ERR_P2P_CP_OFFLINE |
            EXT_REQ_LINK_P2P_REJECTED_SC|
            EXT_P2P_SYNC_VERIFY_CODE_SC|
            EXT_P2P_WAIT_CONNECT_SC |
            EXT_P2P_TRY_CONNECT_SC |
            EXT_P2P_NOTIFY_RELAY_SC |
            EXT_P2P_RELAY_MSG_SC|
            EXT_ERR_P2P_WAIT_ACCEPT_TIMEOUT
            => {
                false
            }
            _ => {true}
        }{ return; }

        let mut stream = Stream::new(_msg.msg);
        let mut cpid = None;
        if Self::need_parse_cpid(_msg.ext){
            if let Some(id) = usize::stream_parse(&mut stream)
            {
                cpid = Some(id);
            }else{
                println!("需要cpid 但是解析失败 {}",_msg.ext);
                return;
            }
        }

        match _msg.ext{
            EXT_REQ_LINK_P2P_SC => {
                let cpid = cpid.unwrap();
                println!("收到请求p2p连接 cp {}",cpid);
                self.add_link(LinkState::WaitAccept, cpid).await;
                if let Some(ref f) = self.event{
                    f.on_recv_p2p_req(cpid).await;
                }else {
                    //先自动同意
                    self.accept_p2p(cpid, true).await;
                }
            }
            EXT_ERR_P2P_WAIT_ACCEPT_TIMEOUT|
            EXT_ERR_P2P_LINK_FAILED|
            EXT_ERR_P2P_CP_OFFLINE |
            EXT_REQ_LINK_P2P_REJECTED_SC => {
                let cpid = cpid.unwrap();
                println!("客户端删除link cp {}",cpid);
                if let Some(link) = self.rm_link(cpid).await
                {
                    if let Some(addr) = link.relay()
                    {
                        let mut relay = self.relay_map.lock().await;
                        let res = relay.remove(&addr);
                        println!("Rm link relay {:?}",res);
                        if let Client(key) = link.local_entity{
                            self.rm_client(key).await;
                        }
                    }
                }
                if _msg.ext == EXT_ERR_P2P_WAIT_ACCEPT_TIMEOUT{
                    if let Some(ref f) = self.event {
                        f.on_recv_wait_accept_timeout(cpid).await;
                    }
                }
            }
            EXT_P2P_SYNC_VERIFY_CODE_SC => {
                let cpid = cpid.unwrap();
                let str = String::from_utf8_lossy(stream.get_rest()).to_string();
                println!("同步验证码 cp {} code {}",cpid,str);

                self.set_verify_code(cpid,str).await;
            }
            EXT_P2P_WAIT_CONNECT_SC => {
                let cpid = cpid.unwrap();
                println!("wait connect");
                self.set_state(cpid,LinkState::WaitingHello1).await;

                self.send_data(cpid.to_be_bytes().to_vec(), EXT_P2P_WAITING_CONNECT_CS).await;
            }
            EXT_P2P_TRY_CONNECT_SC => {
                let cpid = cpid.unwrap();
                if let Some(addr) = Self::parse_addr(&mut stream)
                {
                    if let Some(c) = self.get_verify_code_cpid(cpid).await
                    {
                        println!("try connect {:?}",addr);
                        self.set_state(cpid,LinkState::TryConnect(addr)).await;
                        let data = Self::wrap(c.as_bytes(),Ext::Hello1.into());
                        if let Err(e) = self.send_udp_msg(addr,data.as_slice(),18).await
                        {
                            println!("send udp msg {:?} failed {:?}",Ext::Hello1,e);
                        }
                    }
                }
            }
            EXT_P2P_NOTIFY_RELAY_SC => {
                let cpid = cpid.unwrap();
                if let Some(addr) = Self::parse_addr(&mut stream)
                {
                    if let Some(c) = self.get_verify_code_cpid(cpid).await
                    {
                        println!("建立转发通道to {:?}",addr);
                        let mut d = self.data.lock().await;
                        if let Some(link) = d.link_map.get_mut(&cpid)
                        {
                            link.relay = Some(addr);
                            drop(link);
                            drop(d);
                            let mut relay = self.relay_map.lock().await;
                            relay.insert(addr,cpid);
                            self.send_data(_msg.msg[0..size_of::<usize>()].to_vec(),EXT_P2P_NOTIFY_RELAY_CS).await;
                        }
                    }
                }
            }
            EXT_P2P_RELAY_MSG_SC => {
                if let Some(addr) = Self::parse_addr(&mut stream)
                {
                    self.on_recv_oth_msg(addr,stream.get_rest()).await;
                }
            }
            _=>{}
        }
    }

}

async fn lauch_p2p_ser(
    rx:Receiver<(SocketAddr,Vec<u8>)>,
    sock: Arc<UdpSocket>,
    clients: Arc<Mutex<HashMap<usize,Box<AbClient>>>>,
    plug_data: Arc<Mutex<PlugData>>,
    curr_sender: Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>,
    relay_map: Arc<Mutex<HashMap<SocketAddr,usize>>>,
)
{
    let config = ConfigBuilder::new()
        .thread_count(4)
        .build();

    let mut handler = DefHandler::<TestHandler>::new();
    let parser = DefParser::new();
    let mut plugs = DefPlugMgr::<HeartBeat>::with_time();
    let mut dead_plugs = DefPlugMgr::<HeartBeat>::new();

    {
        handler.add_handler(Arc::new(handlers::heart_beat::HeartbeatHandler{}));
        handler.add_handler(Arc::new(TestHandler{}));
        handler.add_handler(Arc::new(P2PVerifyHandler::new(plug_data.clone(), curr_sender.clone())));
        handler.add_handler(Arc::new(UploadHandler::new()));

        plugs.add_plug(Arc::new(HeartBeat{}));

        dead_plugs.add_plug(Arc::new(P2POnDeadPlugClientSer::new(plug_data,curr_sender.clone(),relay_map.clone())));

    }

    let server = UdpServer::with_clients(
        handler.into(),
        parser.into(),
        plugs.into(),
        dead_plugs.into(),
        config,
        clients
    );
    lazy_static::initialize(&comm::IGNORE_EXT);
    let msg_split_ignore:Option<&'static Vec<u32>> = Some(&comm::IGNORE_EXT);
    let asy_cry_ignore:Option<&'static Vec<u32>> = Some(&comm::IGNORE_EXT);
    //udp_server_run!(server,msg_split_ignore,msg_split_ignore);

    run_udp_server_with_channel(rx,sock,server,msg_split_ignore,asy_cry_ignore,relay_map,curr_sender).await;
}

async fn lauch_p2p_client(
    addr:SocketAddr,
    rx:Receiver<Vec<u8>>,
    sock: Arc<UdpSocket>,
    msg_queue: Arc<Mutex<VecDeque<(Vec<u8>,u32)>>>,
    key: u64,
    plug_data: Arc<Mutex<PlugData>>,
    curr_sender: Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>,
    verify_code: Arc<String>,
    cli_sender_map: Arc<Mutex<HashMap<u64, Sender<Vec<u8>>>>>,
    cli_map: Arc<Mutex<HashMap<u64,(usize,Option<Arc<dyn IUdpClient>>)>>>,
    relay_map: Arc<Mutex<HashMap<SocketAddr,usize>>>
)
{
    let mut handler = client_handler::DefHandler::new();

    {
        let mut msg = msg_queue.lock().await;
        msg.push_back((verify_code.as_bytes().to_vec(),EXT_P2P_CLIENT_VERIFY_CS));
        drop(msg);
        use rcmd_suit::client_handler::Handle;

        handler.add_handler(Arc::new(P2PVerifyClientHandler::new(plug_data.clone(),curr_sender.clone())));
        handler.add_handler(Arc::new(UploadHandler::new()));
    }

    {
        let client = Arc::new( UdpClient::with_msg_queue(
            addr,
            Arc::new(handler),
            DefParser::new(),
            msg_queue.clone()
        ));
        P2PPlug::attached_client_ex(cli_map.clone(),key,client.clone()).await;
        lazy_static::initialize(&comm::IGNORE_EXT);
        let msg_split_ignore:Option<&Vec<u32>> = Some(&comm::IGNORE_EXT);
        let sender = Arc::new(DefUdpSender::New(sock.clone(),addr));
        client.run_with_sender::<P2PPlug,_>(rx,
            msg_split_ignore,msg_split_ignore,
            async {
                let mut cli_sender_map = cli_sender_map.lock().await;
                cli_sender_map.remove(&key);
                drop(cli_sender_map);
                let mut cli_map = cli_map.lock().await;
                if let Some(v) = cli_map.remove(&key)
                {
                    let mut d = plug_data.lock().await;
                    if let Some(link) = d.rm_link(v.0)
                    {
                        if let Some(addr) = link.relay {
                            let mut relay = relay_map.lock().await;
                            let res = relay.remove(&addr);
                            println!("Rm link relay {:?}",res);
                        }
                    }
                }
            },sender).await;
    }
}

async fn lauch_p2p_client_relay(
    addr:SocketAddr,
    rx:Receiver<Vec<u8>>,
    msg_queue: Arc<Mutex<VecDeque<(Vec<u8>,u32)>>>,
    key: u64,
    cpid: usize,
    plug_data: Arc<Mutex<PlugData>>,
    curr_sender: Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>,
    verify_code: Arc<String>,
    cli_sender_map: Arc<Mutex<HashMap<u64, Sender<Vec<u8>>>>>,
    cli_map: Arc<Mutex<HashMap<u64,(usize,Option<Arc<dyn IUdpClient>>)>>>,
    relay_map: Arc<Mutex<HashMap<SocketAddr,usize>>>
)
{
    let mut handler = client_handler::DefHandler::new();

    {
        let mut msg = msg_queue.lock().await;
        msg.push_back((verify_code.as_bytes().to_vec(),EXT_P2P_CLIENT_VERIFY_CS));
        drop(msg);
        use rcmd_suit::client_handler::Handle;

        handler.add_handler(Arc::new(P2PVerifyClientHandler::new(plug_data.clone(),curr_sender.clone())));
        handler.add_handler(Arc::new(UploadHandler::new()));
    }

    {
        let client = Arc::new( UdpClient::with_msg_queue(
            addr,
            Arc::new(handler),
            DefParser::new(),
            msg_queue.clone()
        ));
        P2PPlug::attached_client_ex(cli_map.clone(),key,client.clone()).await;
        lazy_static::initialize(&comm::IGNORE_EXT);
        let msg_split_ignore:Option<&Vec<u32>> = Some(&comm::IGNORE_EXT);
        let sender = Arc::new(AttchedUdpSender::new(curr_sender,
                                                    EXT_P2P_RELAY_MSG_CS,cpid));
        client.run_with_sender::<P2PPlug,_>(rx,
            msg_split_ignore,msg_split_ignore,
            async {
                let mut cli_sender_map = cli_sender_map.lock().await;
                cli_sender_map.remove(&key);
                drop(cli_sender_map);
                let mut cli_map = cli_map.lock().await;
                if let Some(v) = cli_map.remove(&key)
                {
                    let mut d = plug_data.lock().await;
                    if let Some(link) = d.rm_link(v.0)
                    {
                        if let Some(addr) = link.relay {
                            let mut relay = relay_map.lock().await;
                            let res = relay.remove(&addr);
                            println!("Rm link relay {:?}",res);
                        }
                    }
                }
            },sender).await;
    }
}