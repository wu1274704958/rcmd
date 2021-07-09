use std::sync::Arc;
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
#[async_trait]
impl Plug for P2POnDeadPlug {
    type ABClient = AbClient;
    type Id = usize;
    type Config = Config;

    async fn run(&self, id: Self::Id, clients: &Arc<Mutex<HashMap<Self::Id, Box<Self::ABClient>>>>, config: Arc<Self::Config>) where Self::Id: Copy {
        if let Some(k) = self.data.find_key(id).await
        {
            if let Some(d) = self.data.remove(&k).await{
                println!("Remove LinkData {:?}",&d);
                if let LinkState::Failed = d.state() {
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

    async fn run(&self, id: Self::Id, clients: &Arc<Mutex<HashMap<Self::Id, Box<Self::ABClient>>>>, config: Arc<Self::Config>) where Self::Id: Copy {

        if let Some(k) = self.data.find_key(id).await
        {
            let m = self.data.map();
            let mut map = m.lock().await;
            let mut need_rm = false;
            if let Some(d) = map.get_mut(&k)
            {
                let a = d.a();
                let b = d.b();
                let mut cls = clients.lock().await;
                let both_alive = cls.contains_key(&a) && cls.contains_key(&b);
                need_rm = !both_alive;
                
                if !both_alive {
                    if let Some(c) = cls.get_mut(&a)
                    {
                        c.push_msg(b.to_be_bytes().to_vec(), EXT_ERR_P2P_CP_OFFLINE);
                    }
                    if let Some(c) = cls.get_mut(&b)
                    {
                        c.push_msg(a.to_be_bytes().to_vec(), EXT_ERR_P2P_CP_OFFLINE);
                    }
                }else{
                    if let LinkState::Agreed = d.state() 
                    {
                        let verify_code = d.verify_code().as_bytes().to_vec();
                        if let Some(c) = cls.get_mut(&a)
                        {
                            c.push_msg(verify_code.clone(), EXT_P2P_SYNC_VERIFY_CODE_SC);
                        }
                        if let Some(c) = cls.get_mut(&b)
                        {
                            c.push_msg(verify_code, EXT_P2P_SYNC_VERIFY_CODE_SC);
                        }
                    }
                    match d.next_state(clients).await {

                        Some(LinkState::TryConnectBToA(addr_id,addr,time , times)) => {
                            
                            if let Some(c) = cls.get_mut(&b)
                            {
                                let mut data = Vec::<u8>::new();
                                if let std::net::IpAddr::V4(ip) = addr.ip(){
                                    data.extend_from_slice(ip.octets().as_ref());
                                }
                                data.extend_from_slice(addr.port().to_be_bytes().as_ref());
                                c.push_msg(data, EXT_P2P_TRY_CONNECT_SC);
                            }
                        }
                        Some(LinkState::TryConnectAToB(addr_id,addr,time , times)) => {
                            if let Some(c) = cls.get_mut(&a)
                            {
                                let mut data = Vec::<u8>::new();
                                if let std::net::IpAddr::V4(ip) = addr.ip(){
                                    data.extend_from_slice(ip.octets().as_ref());
                                }
                                data.extend_from_slice(addr.port().to_be_bytes().as_ref());
                                c.push_msg(data, EXT_P2P_TRY_CONNECT_SC);
                            }
                        }
                        Some(LinkState::Failed)=>{
                            need_rm = true;
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

