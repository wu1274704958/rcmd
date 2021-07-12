use rcmd_suit::{client_plug::client_plug::ClientPlug, utils::stream_parser::{Stream,StreamParse}};
use std::{collections::{HashMap, VecDeque}, net::{Ipv4Addr}, sync::{Arc,Weak}, usize};
use tokio::net::UdpSocket;
use rcmd_suit::utils::udp_sender::USErr;
use async_trait::async_trait;
use std::net::{SocketAddr, IpAddr};
use tokio::sync::Mutex;
use num_enum::TryFromPrimitive;

use crate::extc::*;
use std::mem::size_of;
use std::convert::TryFrom;
use std::process::abort;

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
#[derive(Debug)]
pub enum P2PErr {
    LinkExist,
    NotFindAnyLocalAddr,
    NotReady,
    BadState,
    Wrap((i32,String))
}

impl From<std::io::Error> for P2PErr
{
    fn from(e: std::io::Error) -> Self {
        let code = match e.raw_os_error() {
            None => {-3}
            Some(v) => {v}
        };
        let str = format!("{}",e);
        P2PErr::Wrap((code,str))
    }
}

#[derive(Eq, PartialEq,Debug)]
enum LinkState{
    WaitAccept,
    Accepted,
    ReadyToTryConnect,
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

    pub fn get_local_ip() -> Vec<Ipv4Addr>
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


    fn need_parse_cpid(ext:u32) -> bool
    {
        ext == EXT_REQ_LINK_P2P_SC ||
        ext == EXT_ERR_P2P_LINK_FAILED||
        ext == EXT_ERR_P2P_CP_OFFLINE ||
        ext == EXT_REQ_LINK_P2P_REJECTED_SC ||
        ext == EXT_P2P_SYNC_VERIFY_CODE_SC ||
        ext == EXT_P2P_WAIT_CONNECT_SC ||
        ext == EXT_P2P_TRY_CONNECT_SC
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

    async fn send_udp_msg(&self,addr:SocketAddr,msg:&[u8],times:u8) -> Result<(),P2PErr>
    {
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
            _ => {
                abort()
            }
        }
    }

    fn get_next_st(ext:Ext) -> LinkState
    {
        match ext {
            Ext::Hello1 => LinkState::WaitingHello3,
            Ext::Hello2 => LinkState::Stage1Success(false),
            Ext::Hello3 => LinkState::Stage1Success(true),
            Ext::Hello4 => LinkState::PrepareRealLink,
            _ => {
                abort()
            }
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
                                    }
                                    Ext::Hello4 => {
                                        self.send_data(link.cp.to_be_bytes().to_vec(),EXT_P2P_CONNECT_SUCCESS_STAGE1_CS).await;
                                        //准备p2p client
                                        println!("准备p2p client");
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

                _ => {}
            }
        }else{

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
            EXT_P2P_TRY_CONNECT_SC => {
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
                //先自动同意
                self.accept_p2p(cpid, true).await;
            }
            EXT_ERR_P2P_LINK_FAILED|
            EXT_ERR_P2P_CP_OFFLINE |
            EXT_REQ_LINK_P2P_REJECTED_SC => {
                let cpid = cpid.unwrap();
                println!("客户端删除link cp {}",cpid);
                self.rm_link(cpid).await;
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
            _=>{}
        }
    }

}