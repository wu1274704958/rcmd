use crate::handler::{Handle, SubHandle};
use std::collections::hash_map::RandomState;
use async_std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::Mutex;
use crate::ab_client::AbClient;
use std::time::SystemTime;
use async_trait::async_trait;

pub struct HeartbeatHandler
{

}
#[async_trait]
impl SubHandle for HeartbeatHandler
{
    type ABClient = AbClient;
    type Id = usize;

    async fn handle(&self, data: &[u8], len: u32, ext: u32, clients: &Arc<Mutex<HashMap<Self::Id, Box<Self::ABClient>, RandomState>>>, id: Self::Id) -> Option<(Vec<u8>,u32)> where Self::Id: Copy {

        let mut a = clients.lock().await;
        if let Some(c) = a.get_mut(&id)
        {
            c.heartbeat_time = SystemTime::now();
            if ext == 9 && data.len() == 1 && data[0] == 9
            {
                return Some((vec![9],9));
            }
        }
        None
    }
}