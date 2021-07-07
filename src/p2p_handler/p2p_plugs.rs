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

    fn exec_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f32(1f32))
    }


    async fn run(&self, id: Self::Id, clients: &Arc<Mutex<HashMap<Self::Id, Box<Self::ABClient>>>>, config: Arc<Self::Config>) where Self::Id: Copy {
        if let Some(k) = self.data.find_key(id).await
        {
            if let Some(d) = self.data.remove(&k).await{
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
            if let Some(d) = map.get_mut(&k)
            {
                match d.state() {
                    LinkState::TryConnectBToA(addr_id,addr,time , times) => {

                    }
                    LinkState::TryConnectAToB(addr_id,addr,time , times) => {

                    }
                    _ => {}
                }
            }
        }
    }
}

