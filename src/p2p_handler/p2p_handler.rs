use rcmd_suit::handler::SubHandle;
use std::sync::Arc;
use std::collections::HashMap;
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

#[derive(Copy, Clone)]
enum LinkState{
    WaitResponse,
    Agreed,
    TryConnectBToA(u8,SocketAddr,SystemTime,u8),
    TryConnectAToB(u8,SocketAddr,SystemTime,u8),
    Failed,
    Success(usize)
}

pub struct LinkData{
    a:usize,
    b:usize,
    state:LinkState,
    key:String,
    verify_code:String,
    local_addr: HashMap<usize,Vec<SocketAddr>>,
    timeout:Duration
}

pub struct P2PLinkData
{
    pub map: Mutex<HashMap<String,LinkData>>,
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
        for i in 0..addr_len{
            let mut arr = [0u8;4];
            for x in 0..4{
               if let Some(v) = stream.next()
               {
                    arr[i as usize] = v;
               }else{
                   return None;
               }
            }
            vs.push( SocketAddr::new(IpAddr::from(arr),prot) );
        }

        Some(vs)
    }
}

#[async_trait]
impl SubHandle for P2PHandlerSer{
    type ABClient = AbClient;
    type Id = usize;

    async fn handle(&self, data: &[u8], len: u32, ext: u32, clients: &Arc<Mutex<HashMap<Self::Id, Box<Self::ABClient>>>>, id: Self::Id) -> Option<(Vec<u8>, u32)> where Self::Id: Copy {
        match ext {
            EXT_REQ_HELP_LINK_P2P_CS => {
                let mut stream = Stream::new(data);
                let cp_lid = match usize::stream_parse(&mut stream)
                {
                    Some(v) =>{ v }
                    None => { return Some((EXT_AGREEMENT_ERR_CODE.to_be_bytes().to_vec(),EXT_REQ_HELP_LINK_P2P_SC)); }
                };
                let mut cls = clients.lock().await;
                let mut cp = if let Some(v) = cls.get_mut(&cp_lid) {v}else{
                    return Some((EXT_ERR_NOT_FOUND_LID.to_be_bytes().to_vec(),EXT_REQ_HELP_LINK_P2P_SC));
                };

                let key = LinkData::gen_key(id.clone(),cp_lid);
                let mut map = self.data.map.lock().await;
                if map.contains_key(&key)
                {
                    return Some((EXT_ERR_LINK_DATA_ALREADY_EXIST.to_be_bytes().to_vec(),EXT_REQ_HELP_LINK_P2P_SC));
                }
                let addrs = if let Some(v) = self.parse_addr(&mut stream)
                {
                    v
                }else {
                    return Some((EXT_AGREEMENT_ERR_CODE.to_be_bytes().to_vec(), EXT_REQ_HELP_LINK_P2P_SC));
                };

                let mut local_addr_map = HashMap::new();
                local_addr_map.insert(id.clone(),addrs);

                let data = LinkData::new(id.clone(),cp_lid,key.clone(),local_addr_map);

                map.insert(key,data);

                drop(map);

                cp.push_msg(id.to_be_bytes().to_vec(),EXT_REQ_LINK_P2P_SC);

                Some((0u32.to_be_bytes().to_vec(),EXT_REQ_HELP_LINK_P2P_SC))
            }
            EXT_REQ_LINK_P2P_CS => {
                let mut stream = Stream::new(data);
                let mut respo = data[0..size_of::<usize>()].to_vec();
                let cp_lid = match usize::stream_parse(&mut stream)
                {
                    Some(v) =>{ v }
                    None => { return Some((EXT_AGREEMENT_ERR_CODE.to_be_bytes().to_vec(),EXT_REQ_LINK_P2P_SC)); }
                };
                let accept = if let Some(v) = stream.next(){v == 1}
                else{
                    return Some((EXT_AGREEMENT_ERR_CODE.to_be_bytes().to_vec(),EXT_REQ_LINK_P2P_SC));
                };
                let key = LinkData::gen_key(id.clone(),cp_lid);
                let mut map = self.data.map.lock().await;
                let mut link_data = if let Some(v) = map.get_mut(&key) {v}
                else{
                    respo.extend_from_slice(EXT_ERR_NOT_FOUND_LINK_DATA.to_be_bytes().as_ref());
                    return Some((respo,EXT_REQ_LINK_P2P_SC));
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
                            return Some((respo,EXT_REQ_LINK_P2P_SC));
                        };
                        link_data.set_local_addr(id.clone(),addrs);
                    }
                    else{
                        map.remove(&key);
                        //通知被拒绝
                        let mut cls = clients.lock().await;
                        if let Some(v) = cls.get_mut(&cp_lid) {
                            v.push_msg(respo.clone(),EXT_REQ_LINK_P2P_REJECTED_SC);
                        }
                    }
                    respo.extend_from_slice(0u32.to_be_bytes().as_ref());
                    return Some((respo,EXT_REQ_LINK_P2P_SC));
                }else{
                    respo.extend_from_slice(EXT_ERR_ALREADY_ACCEPT.to_be_bytes().as_ref());
                    return Some((respo,EXT_REQ_LINK_P2P_SC));
                }
            }
            _ => {None}
        }
    }
    fn interested(&self,ext:u32)->bool{
        ext == EXT_REQ_HELP_LINK_P2P_CS
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
    pub fn new(a:usize,b:usize,key:String,local_addr_map:HashMap<usize,Vec<SocketAddr>>) -> LinkData
    {
        let verify_code = Self::gen_verify_code(&key);
        LinkData{
            a,b,
            state:LinkState::WaitResponse,
            key,
            verify_code,
            local_addr:local_addr_map,
            timeout:Duration::from_secs(2)
        }
    }

    pub fn a(&self) -> usize {
        self.a
    }
    pub fn b(&self) -> usize {
        self.b
    }
    pub fn state(&self) -> &LinkState {
        &self.state
    }
    pub fn key(&self) -> &String {
        &self.key
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

    pub async fn next_state(&mut self,clients: &Arc<Mutex<HashMap<usize, Box<AbClient>>>>) -> Option<LinkState>
    {
        match self.state {
            LinkState::Agreed => {
                let cls =  clients.lock().await;
                let a_addr = if let Some(v) = cls.get(&self.a){ v.addr }
                else{
                    self.state = Failed;
                    return Some(Failed);
                };
                let b_addr = if let Some(v) = cls.get(&self.b){ v.addr }
                else{
                    self.state = Failed;
                    return Some(Failed);
                };
                if a_addr.ip().eq(&b_addr.ip())//外网地址一致 可能在同一局域网下
                {
                    let addrs = self.get_local_addr(self.a).unwrap();
                    self.state = LinkState::TryConnectBToA(0,addrs[0],SystemTime::now(),1);
                }else{
                    if self.eq_local_addr(self.b,b_addr.ip()){
                        self.swap_ab();
                    }
                    self.state = LinkState::TryConnectBToA(100,a_addr,SystemTime::now(),1);
                }
                return Some(self.state);
            }
            LinkState::TryConnectBToA(_, _, _, _) => {}
            LinkState::TryConnectAToB(_, _, _, _) => {}

            _ => {}
        }
        None
    }
}