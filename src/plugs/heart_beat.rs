use crate::plug::Plug;
use std::collections::hash_map::RandomState;
use async_std::sync::Arc;
use std::collections::HashMap;
use std::sync::Mutex;
use crate::ab_client::AbClient;
use crate::config_build::Config;
use std::time::SystemTime;

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
        //if
    }
}