use rcmd_suit::handler::SubHandle;
use std::process::abort;
use std::sync::Arc;
use std::collections::HashMap;
use std::{u32, u8, usize};
use tokio::sync::Mutex;
use async_trait::async_trait;
use rcmd_suit::ab_client::AbClient;
use std::net::{SocketAddr, Ipv4Addr, IpAddr, SocketAddrV4};
use std::time::{SystemTime, Duration};
use rcmd_suit::tools;
use crate::extc::*;
use rcmd_suit::utils::stream_parser::{Stream,StreamParse};
use std::mem::size_of;
use crate::p2p_handler::p2p_handler::LinkState::Failed;
use std::ptr::eq;

#[derive(Copy, Clone,Debug)]
pub enum LinkState{
    WaitResponse,
    Agreed,
    TryConnectBToA(u8,SocketAddr,SystemTime,u8,u8),
    TryConnectAToB(u8,SocketAddr,SystemTime,u8,u8),
    Failed,
    Step1Success(usize,u8,SystemTime),
    Step2Success(usize,u8),
    NotifyConstructRelay(u8,SystemTime,u8)
}

#[derive(Debug)]
pub struct LinkData{
    a:usize,
    b:usize,
    state:LinkState,
    key:u128,
    verify_code:String,
    local_addr: HashMap<usize,Vec<SocketAddr>>,
    timeout:Duration,
    a_addr: SocketAddr,
    b_addr: SocketAddr,
    relay: Option<u8>
}

pub struct P2PLinkData
{
    map: Mutex<HashMap<u128,LinkData>>,
    key_map: Mutex<HashMap<usize,u128>>
}

pub struct P2PHandlerSer{
    data: Arc<P2PLinkData>
}

impl P2PHandlerSer{
    pub fn new(data:Arc<P2PLinkData>) -> P2PHandlerSer
    {
        P2PHandlerSer{data}
    }

    fn parse_addr(&self,stream:&mut Stream) -> Option<Vec<SocketAddr>>
    {
        let addr_len = match stream.next()
        {
            Some(v) =>{ v }
            None => { return None; }
        };
        if addr_len == 0 {return None;}
        let prot = match u16::stream_parse(stream)
        {
            Some(v) =>{ v }
            None => { return None; }
        };
        let mut vs = Vec::with_capacity(addr_len as usize);
        for _i in 0..addr_len{
            let mut arr = [0u8;4];
            for _x in 0..4{
               if let Some(v) = stream.next()
               {
                    arr[_x as usize] = v;
               }else{
                   return None;
               }
            }
            vs.push( SocketAddr::new(IpAddr::from(arr),prot) );
        }

        Some(vs)
    }

    fn get_respones_ext(e:u32) -> u32
    {
        match e {
            EXT_REQ_HELP_LINK_P2P_CS => EXT_REQ_HELP_LINK_P2P_SC,
            EXT_ACCEPT_LINK_P2P_CS =>   EXT_ACCEPT_LINK_P2P_SC,
            EXT_P2P_CONNECT_SUCCESS_STAGE1_CS =>  EXT_P2P_CONNECT_SUCCESS_STAGE1_SC,
            EXT_P2P_CONNECT_SUCCESS_CS =>   EXT_P2P_CONNECT_SUCCESS_SC,
            EXT_P2P_WAITING_CONNECT_CS=> EXT_P2P_WAITING_CONNECT_SC,
            EXT_P2P_CLIENT_DISCONNECT_CS => EXT_P2P_CLIENT_DISCONNECT_SC,
            _=>{abort()}
        }
    }
}

#[async_trait]
impl SubHandle for P2PHandlerSer{
    type ABClient = AbClient;
    type Id = usize;

    async fn handle(&self, data: &[u8], _len: u32, ext: u32, clients: &Arc<Mutex<HashMap<Self::Id, Box<Self::ABClient>>>>, id: Self::Id) -> Option<(Vec<u8>, u32)> where Self::Id: Copy {
        let mut stream = Stream::new(data);
        let mut respo = data[0..size_of::<usize>()].to_vec();
        let cp_lid = match usize::stream_parse(&mut stream)
        {
            Some(v) =>{ v }
            None => { 
                respo.extend_from_slice(EXT_AGREEMENT_ERR_CODE.to_be_bytes().as_ref());
                return Some((respo,Self::get_respones_ext(ext))); 
            }
        };
        if cp_lid == id{
            respo.extend_from_slice(EXT_ERR_P2P_BAD_REQUEST.to_be_bytes().as_ref());
            return Some((respo,Self::get_respones_ext(ext)));
        }
        let mut cls = clients.lock().await;
        let self_addr = cls.get(&id).unwrap().addr;
        let cp = if let Some(v) = cls.get_mut(&cp_lid) {v}else{
            return Some((respo, EXT_ERR_P2P_CP_OFFLINE));
        };
        
        
        match ext {
            EXT_REQ_HELP_LINK_P2P_CS => {
            
                let addrs = if let Some(v) = self.parse_addr(&mut stream)
                {
                    v
                }else {
                    respo.extend_from_slice(EXT_AGREEMENT_ERR_CODE.to_be_bytes().as_ref());
                    return Some((respo, EXT_REQ_HELP_LINK_P2P_SC));
                };

                if self.data.insert(id.clone(),cp_lid,addrs,self_addr,cp.addr).await.is_none()
                {
                    respo.extend_from_slice(EXT_ERR_LINK_DATA_ALREADY_EXIST.to_be_bytes().as_ref());
                    return Some((respo,EXT_REQ_HELP_LINK_P2P_SC));
                }

                cp.push_msg(id.to_be_bytes().to_vec(),EXT_REQ_LINK_P2P_SC);
                respo.extend_from_slice(0u32.to_be_bytes().as_ref());
                Some((respo,EXT_REQ_HELP_LINK_P2P_SC))
            }
            EXT_ACCEPT_LINK_P2P_CS => {
                
                let accept = if let Some(v) = stream.next(){v == 1}
                else{
                    respo.extend_from_slice(EXT_AGREEMENT_ERR_CODE.to_be_bytes().as_ref());
                    return Some((respo,EXT_ACCEPT_LINK_P2P_SC));
                };
                let key = LinkData::gen_key(id.clone(),cp_lid);
                let mut map = self.data.map.lock().await;
                let link_data = if let Some(v) = map.get_mut(&key) {v}
                else{
                    respo.extend_from_slice(EXT_ERR_NOT_FOUND_LINK_DATA.to_be_bytes().as_ref());
                    return Some((respo,EXT_ACCEPT_LINK_P2P_SC));
                };
                if let LinkState::WaitResponse = link_data.state()
                {
                    if accept {
                        link_data.set_state(LinkState::Agreed);
                        let addrs = if let Some(v) = self.parse_addr(&mut stream)
                        {
                            v
                        }else {
                            respo.extend_from_slice(EXT_AGREEMENT_ERR_CODE.to_be_bytes().as_ref());
                            return Some((respo,EXT_ACCEPT_LINK_P2P_SC));
                        };
                        link_data.set_local_addr(id.clone(),addrs);
                    }
                    else{
                        drop(map);
                        self.data.remove(&key).await;
                        //通知被拒绝
                        cp.push_msg(id.to_be_bytes().to_vec(),EXT_REQ_LINK_P2P_REJECTED_SC);
                    }
                    respo.extend_from_slice(0u32.to_be_bytes().as_ref());
                    return Some((respo,EXT_ACCEPT_LINK_P2P_SC));
                }else{
                    respo.extend_from_slice(EXT_ERR_ALREADY_ACCEPT.to_be_bytes().as_ref());
                    return Some((respo,EXT_ACCEPT_LINK_P2P_SC));
                }
            }
            EXT_P2P_CONNECT_SUCCESS_STAGE1_CS => {
                println!("Recv Stage1 success!!");
                let key = LinkData::gen_key(id.clone(),cp_lid);
                let mut map = self.data.map.lock().await;
                let link_data = if let Some(v) = map.get_mut(&key) {v}
                else{
                    respo.extend_from_slice(EXT_ERR_NOT_FOUND_LINK_DATA.to_be_bytes().as_ref());
                    return Some((respo,EXT_P2P_CONNECT_SUCCESS_STAGE1_SC));
                };

                match link_data.state() {
                    LinkState::TryConnectBToA(addr_id, _, _, _,_) |
                    LinkState::TryConnectAToB(addr_id, _, _, _,_) => {
                        link_data.state = LinkState::Step1Success(cp_lid,addr_id,SystemTime::now());
                    }
                    _ => {
                        respo.extend_from_slice(EXT_ERR_P2P_BAD_REQUEST.to_be_bytes().as_ref());
                        return Some((respo,EXT_P2P_CONNECT_SUCCESS_STAGE1_SC));
                    }
                }

                respo.extend_from_slice(0u32.to_be_bytes().as_ref());
                Some((respo,EXT_P2P_CONNECT_SUCCESS_STAGE1_SC))
            }
            EXT_P2P_CONNECT_SUCCESS_CS => {

                let key = LinkData::gen_key(id.clone(),cp_lid);
                let mut map = self.data.map.lock().await;
                let link_data = if let Some(v) = map.get_mut(&key) {v}
                else{
                    respo.extend_from_slice(EXT_ERR_NOT_FOUND_LINK_DATA.to_be_bytes().as_ref());
                    return Some((respo,EXT_P2P_CONNECT_SUCCESS_SC));
                };

                match link_data.state() {
                    LinkState::Step1Success(lid, addr_id, _) => {
                        link_data.state = LinkState::Step2Success(lid,addr_id);
                    }
                    _ => {
                        respo.extend_from_slice(EXT_ERR_P2P_BAD_REQUEST.to_be_bytes().as_ref());
                        return Some((respo,EXT_P2P_CONNECT_SUCCESS_SC));
                    }
                }

                respo.extend_from_slice(0u32.to_be_bytes().as_ref());
                Some((respo,EXT_P2P_CONNECT_SUCCESS_SC))
            }
            EXT_P2P_WAITING_CONNECT_CS => {
                let key = LinkData::gen_key(id.clone(),cp_lid);
                let mut map = self.data.map.lock().await;
                let link_data = if let Some(v) = map.get_mut(&key) {v}
                else{
                    respo.extend_from_slice(EXT_ERR_NOT_FOUND_LINK_DATA.to_be_bytes().as_ref());
                    return Some((respo,Self::get_respones_ext(ext)));
                };

                match link_data.state() {
                    
                    LinkState::TryConnectBToA(addr_id, addr,_, times, 0)  => {
                        
                        if let Some(c) = cls.get_mut(&link_data.b)
                        {
                            let mut data = link_data.a.to_be_bytes().to_vec();
                            if let std::net::IpAddr::V4(ip) = addr.ip(){
                                data.extend_from_slice(ip.octets().as_ref());
                            }
                            data.extend_from_slice(addr.port().to_be_bytes().as_ref());
                            c.push_msg(data, EXT_P2P_TRY_CONNECT_SC);
                        }
                        link_data.state = LinkState::TryConnectBToA(addr_id,addr,SystemTime::now(),times,1);
                    }
                    LinkState::TryConnectAToB(addr_id, addr,_, times, 0) => {
                        if let Some(c) = cls.get_mut(&link_data.a)
                        {
                            let mut data = link_data.b.to_be_bytes().to_vec();
                            if let std::net::IpAddr::V4(ip) = addr.ip(){
                                data.extend_from_slice(ip.octets().as_ref());
                            }
                            data.extend_from_slice(addr.port().to_be_bytes().as_ref());
                            c.push_msg(data, EXT_P2P_TRY_CONNECT_SC);
                        }
                        link_data.state = LinkState::TryConnectAToB(addr_id,addr,SystemTime::now(),times,1);
                    }
                    
                     _ => {
                         respo.extend_from_slice(EXT_ERR_P2P_BAD_REQUEST.to_be_bytes().as_ref());
                         return Some((respo,Self::get_respones_ext(ext)));
                     }
                }
                respo.extend_from_slice(0u32.to_be_bytes().as_ref());
                return Some((respo,Self::get_respones_ext(ext)));
            }

            EXT_P2P_CLIENT_DISCONNECT_CS => {
                let key = LinkData::gen_key(id.clone(),cp_lid);
                let mut map = self.data.map.lock().await;
                let link_data = if let Some(v) = map.get_mut(&key) {v}
                else{
                    respo.extend_from_slice(EXT_ERR_NOT_FOUND_LINK_DATA.to_be_bytes().as_ref());
                    return Some((respo,Self::get_respones_ext(ext)));
                };

                match link_data.state() {
                    
                    LinkState::Step2Success(ser_id,_)  => {
                        if ser_id == id {
                            drop(link_data);
                            drop(map);
                            self.data.remove(&key).await;
                        }else{
                            respo.extend_from_slice(EXT_ERR_P2P_BAD_REQUEST.to_be_bytes().as_ref());
                            return Some((respo,Self::get_respones_ext(ext)));
                        }
                    }
                     _ => {
                         respo.extend_from_slice(EXT_ERR_P2P_BAD_REQUEST.to_be_bytes().as_ref());
                         return Some((respo,Self::get_respones_ext(ext)));
                     }
                }
                respo.extend_from_slice(0u32.to_be_bytes().as_ref());
                return Some((respo,Self::get_respones_ext(ext)));
            }
            
            _ => {None}
        }
    }
    fn interested(&self,ext:u32)->bool{
        ext == EXT_REQ_HELP_LINK_P2P_CS ||
        ext == EXT_ACCEPT_LINK_P2P_CS ||
        ext == EXT_P2P_CONNECT_SUCCESS_STAGE1_CS || 
        ext == EXT_P2P_CONNECT_SUCCESS_CS ||
        ext == EXT_P2P_WAITING_CONNECT_CS || 
        ext == EXT_P2P_CLIENT_DISCONNECT_CS 
    }

}

impl LinkData {
    pub fn gen_key(a:usize,b:usize) -> u128
    {
        return if a > b {
            let r = a as u128;
            (r << Self::get_digits(b)) | b as u128
        } else {
            let r = b as u128;
            (r << Self::get_digits(a)) | a as u128
        }
    }
    fn get_digits(mut n:usize) -> usize
    {
        let k = 1usize << (size_of::<usize>() - 1);
        let mut d = size_of::<usize>();
        loop{
            if d == 0 { break; }
            if (n & k) != 0{
                return d;
            }
            n <<= 1;
            d -= 1;
        }
        d
    }
    pub fn gen_verify_code(key:&u128) -> String
    {
         format!("{}@{}", tools::uuid(),key.to_string())
    }
    pub fn new(a:usize,b:usize,key:u128,local_addr_map:HashMap<usize,Vec<SocketAddr>>,
    a_addr:SocketAddr,b_addr:SocketAddr) -> LinkData
    {
        let verify_code = Self::gen_verify_code(&key);
        LinkData{
            a,b,
            state:LinkState::WaitResponse,
            key,
            verify_code,
            local_addr:local_addr_map,
            timeout:Duration::from_secs(2),
            a_addr,
            b_addr,
            relay: None
        }
    }

    pub fn a(&self) -> usize {
        self.a
    }
    pub fn b(&self) -> usize {
        self.b
    }
    pub fn state(&self) -> LinkState {
        self.state
    }
    #[allow(dead_code)]
    pub fn key(&self) -> u128 {
        self.key
    }
    pub fn verify_code(&self) -> &String {
        &self.verify_code
    }
    pub fn get_local_addr(&self,id:usize) -> Option<&Vec<SocketAddr>>
    {
        return if let Some(v) = self.local_addr.get(&id)
        {
            Some(v)
        }else{ None }
    }

    pub fn swap_ab(&mut self)
    {
        let c = self.a;
        self.a = self.b;
        self.b = c;

        let ca = self.a_addr;
        self.a_addr = self.b_addr;
        self.b_addr = ca;
    }
    pub fn set_state(&mut self, state: LinkState) {
        self.state = state;
    }

    pub fn set_local_addr(&mut self,id:usize,addr:Vec<SocketAddr>) -> Option<Vec<SocketAddr>>
    {
        self.local_addr.insert(id,addr)
    }

    pub fn eq_local_addr(&self,id:usize,ip:IpAddr) -> bool
    {
        if let Some(v) = self.local_addr.get(&id)
        {
            for i in v.iter()
            {
                if i.ip().eq(&ip){ return true;}
            }
        }
        false
    }

    fn try_connect_local_times() -> u8 { 3u8 }
    fn try_connect_times() -> u8 { 10u8 } //必须偶数
    fn try_wait_time() -> Duration { Duration::from_secs(2) }
    fn wait_step2_time() -> Duration { Duration::from_secs(10) }
    //优先取_1的地址
    fn get_local_addr_next_translation(&self,_1:usize,_2:usize,addr_id:u8) -> Option<(SocketAddr,u8,bool)>
    {
        let a_addrs = self.local_addr.get(&_2).unwrap();
        let b_addrs = self.local_addr.get(&_1).unwrap();
        {
            let new_addr_id = addr_id + 1;
            if addr_id as usize >= b_addrs.len() {
                if new_addr_id as usize >= a_addrs.len() {
                    None
                }else{
                    Some((a_addrs[new_addr_id as usize],new_addr_id,true))
                }
            }else{
                Some((b_addrs[addr_id as usize],addr_id,false))
            }
        }
    }

    fn get_local_addr_next_slantdown(&self,_1:usize,_2:usize,addr_id:u8) -> Option<(SocketAddr,u8,bool)>
    {
        let a_addrs = self.local_addr.get(&_2).unwrap();
        let b_addrs = self.local_addr.get(&_1).unwrap();
        {
            let new_addr_id = addr_id + 1;
            if new_addr_id as usize >= b_addrs.len() {
                if new_addr_id as usize >= a_addrs.len() {
                    None
                }else{
                    Some((a_addrs[new_addr_id as usize],new_addr_id,true))
                }
            }else{
                Some((b_addrs[new_addr_id as usize],new_addr_id,false))
            }
        }
    }

    pub async fn next_state(&mut self,_clients: &Arc<Mutex<HashMap<usize, Box<AbClient>>>>) -> Option<LinkState>
    {
        match self.state {
            LinkState::Agreed => {
                if self.a_addr.ip().eq(&self.b_addr.ip())//外网地址一致 可能在同一局域网下
                {
                    let addrs = self.get_local_addr(self.a).unwrap();
                    self.state = LinkState::TryConnectBToA(0,addrs[0],SystemTime::now(),1,0);
                }else{
                    if self.eq_local_addr(self.b,self.b_addr.ip()){
                        self.swap_ab();
                    }
                    self.state = LinkState::TryConnectBToA(100,self.a_addr,SystemTime::now(),1,0);
                }
                return Some(self.state);
            }
            LinkState::TryConnectBToA(addr_id, _,time, mut times,_) => {
                
                let now = SystemTime::now();
                if addr_id == 100
                {
                    if let Ok(d) = now.duration_since(time){
                        if d >= Self::try_wait_time(){
                            times += 1;
                            if times > Self::try_connect_times(){ 
                                //self.set_state(LinkState::Failed);
                                self.state = LinkState::NotifyConstructRelay(100,SystemTime::now(),0);
                            }else{
                                self.set_state(LinkState::TryConnectAToB(100,self.b_addr,now,times,0));
                            }
                            return Some(self.state);
                        }
                    }
                }else{
                    if let Ok(d) = now.duration_since(time){
                        if d >= Self::try_wait_time(){
                            let next = self.get_local_addr_next_translation(self.b, self.a, addr_id);
                            if next.is_none(){
                                if times > Self::try_connect_local_times(){
                                    //self.state = LinkState::Failed;
                                    self.state = LinkState::NotifyConstructRelay(0,SystemTime::now(),0);
                                }else{
                                    let addrs = self.get_local_addr(self.a).unwrap();
                                    self.state = LinkState::TryConnectBToA(0,addrs[0],SystemTime::now(),times + 1,0);
                                }
                            }else{
                                let (addr,aid,same) = next.unwrap();
                                if same {
                                    self.state = LinkState::TryConnectBToA(aid,addr,SystemTime::now(),times,0);
                                }else{
                                    self.state = LinkState::TryConnectAToB(aid,addr,SystemTime::now(),times,0);
                                }
                            }
                            return Some(self.state);
                        }
                    }
                }
            }
            LinkState::TryConnectAToB(addr_id, _,time, mut times,_) => {
                let now = SystemTime::now();
                if addr_id == 100
                {
                    if let Ok(d) = now.duration_since(time){
                        if d >= Self::try_wait_time(){
                            times += 1;
                            self.set_state(LinkState::TryConnectBToA(100,self.a_addr,now,times,0));
                            return Some(self.state);
                        }
                    }
                }else{
                    if let Ok(d) = now.duration_since(time){
                        if d >= Self::try_wait_time(){
                            let next = self.get_local_addr_next_slantdown(self.a, self.b, addr_id);
                            if next.is_none(){
                                if times > Self::try_connect_local_times(){
                                    //self.state = LinkState::Failed;
                                    self.state = LinkState::NotifyConstructRelay(0,SystemTime::now(),0);
                                }else{
                                    let addrs = self.get_local_addr(self.a).unwrap();
                                    self.state = LinkState::TryConnectBToA(0,addrs[0],SystemTime::now(),times + 1,0);
                                }
                            }else{
                                let (addr,aid,same) = next.unwrap();
                                if same {
                                    self.state = LinkState::TryConnectAToB(aid,addr,SystemTime::now(),times,0);
                                }else{
                                    self.state = LinkState::TryConnectBToA(aid,addr,SystemTime::now(),times,0);
                                }
                            }
                            return Some(self.state);
                        }
                    }
                }

            }
            LinkState::Step1Success(_,_,time) =>{
                let now = SystemTime::now();
                if let Ok(d) = now.duration_since(time){
                    if d >= Self::wait_step2_time(){
                        self.set_state(LinkState::Failed);
                        return  Some(self.state);
                    }
                }
            }

            _ => {}
        }
        None
    }
}
#[allow(dead_code)]
impl  P2PLinkData{
    pub fn new()-> P2PLinkData
    {
        P2PLinkData{
            map: Mutex::new(HashMap::new()),
            key_map: Mutex::new(HashMap::new())
        }
    }

    pub async fn contains_key(&self,key:&u128) -> bool
    {
        let map = self.map.lock().await;
        map.contains_key(key)
    }

    pub async fn insert(&self,a:usize,b:usize,addr_of_a:Vec<SocketAddr>,a_addr:SocketAddr,b_addr:SocketAddr) -> Option<u128>
    {
        let key = LinkData::gen_key(a,b);
        let mut map = self.map.lock().await;
        if map.contains_key(&key)
        {
            return None;
        }

        let mut local_addr_map = HashMap::new();
        local_addr_map.insert(a,addr_of_a);

        let data = LinkData::new(a,b,key,local_addr_map,a_addr,b_addr);

        map.insert(key,data);
        drop(map);

        let mut key_map = self.key_map.lock().await;

        key_map.insert(a,key);
        key_map.insert(b,key);

        Some(key)
    }

    pub async fn remove(&self,key:&u128) -> Option<LinkData>
    {
        let mut map = self.map.lock().await;
        let mut key_map = self.key_map.lock().await;
        return if let Some(d) = map.remove(&key){
            key_map.remove(&d.a);
            key_map.remove(&d.b);
            Some(d)
        }else{
            None
        }
    }

    pub async fn find_key(&self,a:usize) -> Option<u128>
    {
        let key_map = self.key_map.lock().await;
        if let Some(k) = key_map.get(&a)
        {
            Some(*k)
        }else{
            None
        }
    }


    pub fn map(&self) -> &Mutex<HashMap<u128, LinkData>> {
        &self.map
    }
}

#[test]
fn test_gen_key()
{
    assert_eq!(LinkData::gen_key(3,7),31);
    assert_eq!(LinkData::gen_key(7,3),31);
    assert_eq!(LinkData::gen_key(63,99),6399);
}
