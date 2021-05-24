use std::time::{Duration, SystemTime};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct DataCurrentLimiter{
    duration:Duration,
    limit:usize,
    up_ratio:usize,
    down_ratio:usize,
    current:Mutex<usize>,
    last_time:Mutex<SystemTime>
}

impl DataCurrentLimiter{
    pub fn new(dur:Duration,limit:usize,up_ratio:usize,down_ratio:usize) ->DataCurrentLimiter
    {
        DataCurrentLimiter{
            duration:dur,
            limit,
            up_ratio,
            down_ratio,
            current:Mutex::new(0),
            last_time :Mutex::new(SystemTime::now())
        }
    }

    pub async fn can_send(&self,size:usize) -> bool
    {
        let mut curr = self.current.lock().await;
        let mut last = self.last_time.lock().await;
        let now = SystemTime::now();
        if let Ok(d) = now.duration_since(*last)
        {
            if d > self.duration
            {
                let res = size <= self.limit;
                if res {  *curr = size; *last = now; }
                return res;
            }else{
                let n = *curr + size;
                let res = n <= self.limit;
                if res { *curr = n;*last = now;}
                return res;
            }
        }
        return false;
    }
}