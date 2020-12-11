use std::net::SocketAddr;
use std::thread::ThreadId;
use tokio::net::TcpStream;
use std::cell::RefCell;
use crate::ab_client::State::Ready;

#[derive(Debug)]
pub enum State
{
    Ready,
    Alive,
    Wait,
    Busy,
    Dead
}
#[derive(Debug)]
pub struct AbClient
{
    pub local_addr: SocketAddr,
    pub addr: SocketAddr,
    pub logic_id:usize,
    pub form_thread:ThreadId,
    pub state:State,
    pub write_buf:Option<Vec<u8>>
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
            write_buf:None
        }
    }
}

