use crate::plug::Plug;
use std::collections::hash_map::RandomState;
use async_std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::Mutex;
use crate::ab_client::AbClient;
use crate::config_build::Config;
use std::time::{SystemTime, Duration};
use crate::servers::tcp_server::set_client_st_ex;
use crate::ab_client::State::WaitKill;
use async_trait::async_trait;
pub struct HeartBeat{

}
#[async_trait]
impl Plug for HeartBeat
{
    type ABClient = AbClient;
    type Id = usize;
    type Config = Config;

    fn exec_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f32(1f32))
    }

    async fn run(&self, id: Self::Id, clients: &Arc<Mutex<HashMap<Self::Id, Box<Self::ABClient>, RandomState>>>, config: Arc<Self::Config>) where Self::Id: Copy {
        let time;
        let now = SystemTime::now();
        {
            let a = clients.lock().await;
            if let Some(c) = a.get(&id)
            {
                time = c.heartbeat_time;
            }else {
                return;
            }
        }
        if let Ok(n) = now.duration_since(time)
        {
            if n > config.heartbeat_dur {
                println!("heartbeat check failed will del client {}",id);
                set_client_st_ex(clients,id,WaitKill).await;
            }
        }
    }
}