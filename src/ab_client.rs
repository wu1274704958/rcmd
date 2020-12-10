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
    pub socket:TcpStream,
    pub state:State
}

impl AbClient {
    pub fn new(local_addr: SocketAddr,addr:SocketAddr,logic_id:usize,form_thread:ThreadId,socket:TcpStream)->AbClient
    {
        AbClient{
            local_addr,
            addr,
            logic_id,
            form_thread,
            socket,
            state:Ready
        }
    }
}

