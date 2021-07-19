use std::{mem::size_of, sync::Arc};
use crate::p2p_handler::p2p_handler::{P2PLinkData,LinkState};
use rcmd_suit::plug::Plug;
use std::collections::HashMap;
use tokio::sync::Mutex;
use rcmd_suit::ab_client::AbClient;
use rcmd_suit::config_build::Config;
use async_trait::async_trait;
use crate::extc::*;
use std::time::{SystemTime, Duration};

pub struct P2POnDeadPlug{
    data: Arc<P2PLinkData>
}

impl P2POnDeadPlug{
    pub fn new(data:Arc<P2PLinkData>)-> P2POnDeadPlug
    {
        P2POnDeadPlug{
            data
        }
    }
}

#[async_trait]
impl Plug for P2POnDeadPlug {
    type ABClient = AbClient;
    type Id = usize;
    type Config = Config;

    async fn run(&self, id: Self::Id, clients: &Arc<Mutex<HashMap<Self::Id, Box<Self::ABClient>>>>, _config: Arc<Self::Config>) where Self::Id: Copy {
        if let Some(k) = self.data.find_key(id).await
        {
            if let Some(d) = self.data.remove(&k).await{
                println!("Remove LinkData {:?}",&d);
                //if let LinkState::Failed = d.state() 
                {
                    let cp = if d.a() == id { d.b() } else { d.a() };
                    let mut cls = clients.lock().await;
                    if let Some(c) = cls.get_mut(&cp)
                    {
                        c.push_msg(id.to_be_bytes().to_vec(),EXT_ERR_P2P_CP_OFFLINE);
                    }
                }
            }
        }
    }
}

pub struct P2PPlug{
    data: Arc<P2PLinkData>
}

impl P2PPlug{
    pub fn new(data:Arc<P2PLinkData>)-> P2PPlug
    {
        P2PPlug{
            data
        }
    }
}

#[async_trait]
impl Plug for P2PPlug {
    type ABClient = AbClient;
    type Id = usize;
    type Config = Config;

    async fn run(&self, id: Self::Id, clients: &Arc<Mutex<HashMap<Self::Id, Box<Self::ABClient>>>>, _config: Arc<Self::Config>) where Self::Id: Copy {

        if let Some(k) = self.data.find_key(id).await
        {
            let m = self.data.map();
            let mut map = m.lock().await;
            let mut need_rm = false;
            if let Some(d) = map.get_mut(&k)
            {
                let a = d.a();
                let b = d.b();
                let a_ = a.to_be_bytes();
                let b_ = b.to_be_bytes();
                let mut cls = clients.lock().await;
                let both_alive = cls.contains_key(&a) && cls.contains_key(&b);
                need_rm = !both_alive;
                
                if !both_alive {
                    if let Some(c) = cls.get_mut(&a)
                    {
                        c.push_msg(b_.to_vec(), EXT_ERR_P2P_CP_OFFLINE);
                    }
                    if let Some(c) = cls.get_mut(&b)
                    {
                        c.push_msg(a_.to_vec(), EXT_ERR_P2P_CP_OFFLINE);
                    }
                }else{
                    if let LinkState::Agreed = d.state() 
                    {
                        let mut verify_code = b.to_be_bytes().to_vec();
                        verify_code.extend_from_slice(d.verify_code().as_bytes());
                        if let Some(c) = cls.get_mut(&a)
                        {
                            c.push_msg(verify_code.clone(), EXT_P2P_SYNC_VERIFY_CODE_SC);
                        }
                        if let Some(c) = cls.get_mut(&b)
                        {
                            (&mut verify_code[0..size_of::<usize>()]).copy_from_slice(a.to_be_bytes().as_ref());
                            c.push_msg(verify_code, EXT_P2P_SYNC_VERIFY_CODE_SC);
                        }
                    }
                    match d.next_state(clients) {

                        Some(LinkState::TryConnectBToA(_addr_id,addr,_time , _times,sub_st)) => {
                            match sub_st {
                                0 => {
                                    if let Some(c) = cls.get_mut(&a)
                                    {
                                        c.push_msg(b_.to_vec(), EXT_P2P_WAIT_CONNECT_SC);
                                    }
                                }
                                1 => {
                                    if let Some(c) = cls.get_mut(&b)
                                    {
                                        let mut data = a_.to_vec();
                                        if let std::net::IpAddr::V4(ip) = addr.ip(){
                                            data.extend_from_slice(ip.octets().as_ref());
                                        }
                                        data.extend_from_slice(addr.port().to_be_bytes().as_ref());
                                        c.push_msg(data, EXT_P2P_TRY_CONNECT_SC);
                                    }
                                }
                                _=>{}
                            }
                        }
                        Some(LinkState::TryConnectAToB(_addr_id,addr,_time , _times,sub_st)) => {
                           
                            match sub_st {
                                0 => {
                                    if let Some(c) = cls.get_mut(&b)
                                    {
                                        c.push_msg(a_.to_vec(), EXT_P2P_WAIT_CONNECT_SC);
                                    }
                                }
                                1 => {
                                    if let Some(c) = cls.get_mut(&a)
                                    {
                                        let mut data = b_.to_vec();
                                        if let std::net::IpAddr::V4(ip) = addr.ip(){
                                            data.extend_from_slice(ip.octets().as_ref());
                                        }
                                        data.extend_from_slice(addr.port().to_be_bytes().as_ref());
                                        c.push_msg(data, EXT_P2P_TRY_CONNECT_SC);
                                    }
                                }
                                _=>{}
                            }
                        }
                        Some(LinkState::NotifyConstructRelay(addr_id,time,st)) =>
                        {
                            if st == 0{
                                let (a_addr,b_addr) = if addr_id >= 100{
                                    (d.a_addr(),d.b_addr())
                                }else{
                                    let av = d.get_local_addr(a);
                                    let bv = d.get_local_addr(b);
                                    if av.is_some() && bv.is_some(){
                                        (av.unwrap()[0],bv.unwrap()[0])
                                    }else{
                                        (d.a_addr(),d.b_addr())
                                    }
                                };
                                if let Some(c) = cls.get_mut(&a)
                                {
                                    let mut data = b_.to_vec();
                                    if let std::net::IpAddr::V4(ip) = b_addr.ip(){
                                        data.extend_from_slice(ip.octets().as_ref());
                                    }
                                    data.extend_from_slice(b_addr.port().to_be_bytes().as_ref());
                                    c.push_msg(data, EXT_P2P_NOTIFY_RELAY_SC);
                                }
                                if let Some(c) = cls.get_mut(&b)
                                {
                                    let mut data = a_.to_vec();
                                    if let std::net::IpAddr::V4(ip) = a_addr.ip(){
                                        data.extend_from_slice(ip.octets().as_ref());
                                    }
                                    data.extend_from_slice(a_addr.port().to_be_bytes().as_ref());
                                    c.push_msg(data, EXT_P2P_NOTIFY_RELAY_SC);
                                }
                                d.set_state(LinkState::NotifyConstructRelay(addr_id,SystemTime::now(),st + 1));
                            }
                        }
                        Some(LinkState::Failed)=>{
                            need_rm = true;
                            if let Some(c) = cls.get_mut(&a)
                            {
                                c.push_msg(b_.to_vec(), EXT_ERR_P2P_LINK_FAILED);
                            }
                            if let Some(c) = cls.get_mut(&b)
                            {
                                c.push_msg(a_.to_vec(), EXT_ERR_P2P_LINK_FAILED);
                            }
                        }
                        _ => {}
                    }
                }
            }
            drop(map);
            if need_rm{
                println!("Remove LinkData {}",&k);
                self.data.remove(&k).await;
            }
        }
    }

    fn exec_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f32(1f32))
    }
}

