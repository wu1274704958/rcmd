use std::net::SocketAddr;
use std::thread::ThreadId;
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

impl State {
    pub fn is_wait_kill(&self) -> bool
    {
        match *self{
            State::Ready|
            State::Alive|
            State::Wait |
            State::Busy |
            State::Dead => false,
            State::WaitKill => true,
        }
    }
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
            q.pop_front()
        }
    }
}

pub trait ABClient
{
    type LID;
    fn create(local_addr: SocketAddr,addr:SocketAddr,logic_id:Self::LID,form_thread:ThreadId)->Self;
    fn push_msg(&self,d:Vec<u8>,ext:u32);
    fn pop_msg(& self)->Option<(Vec<u8>,u32)>;
    fn state(&self) -> State;
    fn local_addr(&self)->SocketAddr;
    fn addr(&self)->SocketAddr;
    fn thread_id(&self)->ThreadId;
    fn heartbeat_time(&self)->SystemTime;
    fn reset_heartbeat(&mut self);
    fn set_state(&mut self,s:State);
}

impl ABClient for AbClient
{
    type LID = usize;

    fn create(local_addr: SocketAddr, addr: SocketAddr, logic_id: Self::LID, form_thread: ThreadId) -> Self {
        Self::new(local_addr,addr,logic_id,form_thread)
    }

    fn push_msg(&self, d: Vec<u8>, ext: u32) {
        self.push_msg(d,ext)
    }

    fn pop_msg(&self) -> Option<(Vec<u8>, u32)> {
        self.pop_msg()
    }

    fn state(&self) -> State {
        self.state
    }

    fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    fn addr(&self) -> SocketAddr {
        self.addr
    }

    fn thread_id(&self) -> ThreadId {
        self.form_thread
    }

    fn heartbeat_time(&self) -> SystemTime {
        self.heartbeat_time
    }

    fn reset_heartbeat(&mut self) {
        self.heartbeat_time = SystemTime::now()
    }

    fn set_state(&mut self, s: State) {
        self.state = s;
    }
}

