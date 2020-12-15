use std::net::{SocketAddr, SocketAddrV4, Ipv4Addr};
use std::str::FromStr;
use tokio::time::Duration;

#[derive(Copy, Clone)]
pub struct Config
{
    pub thread_count:usize,
    pub addr:SocketAddr,
    pub min_sleep_dur:Duration,
    pub max_sleep_dur:Duration,
    pub big_msg_limit:usize,
    pub heartbeat_dur:Duration
}

pub struct ConfigBuilder
{
    config:Config
}

impl ConfigBuilder
{
    pub fn new()->ConfigBuilder
    {
        ConfigBuilder{
            config:Config{
                thread_count:0,
                addr:SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(0,0,0,0),8080)),
                min_sleep_dur: Duration::from_micros(100),
                max_sleep_dur: Duration::from_millis(1),
                big_msg_limit: 1024 * 128,
                heartbeat_dur: Duration::from_secs(5)
            }
        }
    }

    pub fn thread_count(&mut self,v:usize)->&mut ConfigBuilder
    {
        self.config.thread_count = v;
        self
    }

    pub fn addr(&mut self,addr:SocketAddr)->&mut ConfigBuilder
    {
        self.config.addr = addr;
        self
    }

    pub fn addr_s(&mut self,addr:&str)->&mut ConfigBuilder
    {
        if let Ok(n) = SocketAddr::from_str(addr)
        {
            self.config.addr = n;
        }
        self
    }

    pub fn build(&self)->Config
    {
        self.config
    }
}