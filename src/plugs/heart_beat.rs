use crate::plug::Plug;
use std::collections::hash_map::RandomState;
use async_std::sync::Arc;
use std::collections::HashMap;
use std::sync::Mutex;
use crate::ab_client::AbClient;
use crate::config_build::Config;
use std::time::SystemTime;
use crate::tools::set_client_st;
use crate::ab_client::State::WaitKill;

pub struct HeartBeat{

}

impl Plug for HeartBeat
{
    type ABClient = AbClient;
    type Id = usize;
    type Config = Config;

    fn run(&self, id: Self::Id, clients: &mut Arc<Mutex<HashMap<Self::Id, Box<Self::ABClient>, RandomState>>>, config: &Self::Config) where Self::Id: Copy {
        let time;
        {
            let a = clients.lock().unwrap();
            if let Some(c) = a.get(&id)
            {
                time = c.heartbeat_time;
            }else {
                return;
            }
        }
        let now = SystemTime::now();
        if let Ok(n) = now.duration_since(time)
        {
            if n > config.heartbeat_dur {
                println!("heartbeat check failed will del client {}",id);
                set_client_st(clients,id,WaitKill);
            }
        }
    }
}