use std::net::SocketAddr;
use std::thread::ThreadId;
use tokio::net::TcpStream;
use std::cell::RefCell;
use crate::ab_client::State::Ready;
use std::time::SystemTime;
use std::collections::VecDeque;
use std::sync::{Arc,Mutex};

#[derive(Debug,Copy, Clone)]
pub enum State
{
    Ready,
    Alive,
    Wait,
    Busy,
    Dead,
    WaitKill
}
#[derive(Debug)]
pub struct AbClient
{
    pub local_addr: SocketAddr,
    pub addr: SocketAddr,
    pub logic_id:usize,
    pub form_thread:ThreadId,
    pub state:State,
    write_buf:Arc<Mutex<VecDeque<(Vec<u8>,u32)>>>,
    pub heartbeat_time:SystemTime
}

impl AbClient {
    pub fn new(local_addr: SocketAddr,addr:SocketAddr,logic_id:usize,form_thread:ThreadId)->AbClient
    {
        AbClient{
            local_addr,
            addr,
            logic_id,
            form_thread,
            state:Ready,
            write_buf:Arc::new(Mutex::new(VecDeque::new())),
            heartbeat_time:SystemTime::now()
        }
    }

    pub fn push_msg(&self,d:Vec<u8>,ext:u32)
    {
        let mut q = self.write_buf.lock().unwrap();
        q.push_back((d,ext));
    }

    pub fn pop_msg(& self)->Option<(Vec<u8>,u32)>
    {
        let mut q = self.write_buf.lock().unwrap();
        if q.is_empty() {  None }else{
            q.pop_back()
        }
    }
}

