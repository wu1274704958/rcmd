use async_trait::async_trait;
use tokio::net::UdpSocket;
use std::sync::Arc;
use num_traits::{One, Zero};
use std::collections::{VecDeque, HashMap};
use std::mem::size_of;
use async_std::net::SocketAddr;
use crate::subpackage::{UdpSubpackage,Subpackage};
use tokio::time::Duration;
use std::time::SystemTime;
use async_std::io::Error;
use crate::tools::{TOKEN_NORMAL};
use crate::utils::msg_split::UdpMsgSplit;
use tokio::sync::Mutex;

#[async_trait]
pub trait UdpSender{
    async fn send_msg(&self,v:Vec<u8>)->Result<(),USErr>;
    async fn check_recv(&mut self, data: &[u8]) -> Result<Vec<u8>,USErr>;
    fn need_check(&self)->bool;
    fn create(sock:Arc<UdpSocket>,addr:SocketAddr) ->Self;
    async fn set_max_msg_len(&mut self,len:u16);
    fn max_msg_len(&self)->u16;
    async fn set_min_msg_len(&mut self,len:u16);
    fn min_msg_len(&self)->u16;
    fn cache_size(&self)->u16;
    fn set_cache_size(&mut self,s:u16);
    fn set_time_out(&mut self,dur:Duration);
    async fn check_send(&mut self)->Result<(),USErr>;
    fn set_retry_times(&mut self,v:u16);
    fn retry_times(&self)->u16;
}

pub struct DefUdpSender{
    sock: Arc<UdpSocket>,
    max_len: u16,
    min_len: u16,
    cache_size:u16,
    mid: Arc<Mutex<usize>>,
    queue: Arc<Mutex<VecDeque<usize>>>,
    msg_map: Arc<Mutex<HashMap<usize,(Vec<u8>,SystemTime,u16)>>>,
    recv_cache: HashMap<usize,(Vec<u8>,u32,u8)>,
    expect_id: usize,
    addr:SocketAddr,
    subpacker: UdpSubpackage,
    timeout: Duration,
    msg_split: Arc<Mutex<UdpMsgSplit>>,
    max_retry_times: u16,
    msg_cache_queue: Arc<Mutex<VecDeque<(usize,Vec<u8>)>>>
}

pub enum USErr{
    EmptyMsg,
    MsgCacheOverflow,
    RetryTimesLimit,
    Warp(std::io::Error),
    ResponseSendSuccMiss,
    SendSuccMissCache
}

impl USErr {
    pub fn code(&self)->i32
    {
        match self {
            USErr::EmptyMsg => { 0 }
            USErr::MsgCacheOverflow => {-1}
            USErr::RetryTimesLimit => {-2}
            USErr::Warp(e) => { match e.raw_os_error() {
                None => {-3}
                Some(v) => {v}
            }}
            USErr::ResponseSendSuccMiss => {-4},
            USErr::SendSuccMissCache=>{-5}
        }
    }

    pub fn err_str(&self)->String
    {
        match self {
            USErr::EmptyMsg => {"Empty Message".to_string()}
            USErr::MsgCacheOverflow => {"Message cache overflow".to_string()}
            USErr::RetryTimesLimit => {"Retry times reach the maximum".to_string()}
            USErr::Warp(e) => {  format!("{}",e) }
            USErr::ResponseSendSuccMiss => { "Response send success miss message id".to_string() },
            USErr::SendSuccMissCache=>{"Send success miss the cache".to_string()}
        }
    }
}

impl From<std::io::Error> for USErr
{
    fn from(e: Error) -> Self {
        USErr::Warp(e)
    }
}

impl DefUdpSender
{
    async fn get_mid(&self)->usize
    {
        let mut mid = self.mid.lock().await;
        if *mid == usize::max_value()
        {
            *mid = usize::one();
        }else{
            *mid += usize::one();
        }
        *mid
    }

    async fn warp(&self,v:&[u8],ext:u32,tag:u8)->Result<Vec<u8>,USErr>
    {
        let mid = self.get_mid().await;
        //println!("send id {} {:?}",mid,v);
        let res = Self::warp_ex(v,ext,tag,mid);
        self.push_cache(mid,res.clone()).await?;
        Ok(res)
    }

    fn warp_ex(v:&[u8],ext:u32,tag:u8,mid:usize)->Vec<u8>
    {
        assert!(!v.is_empty());
        let len = v.len() + Self::package_len();
        let mut res = Vec::with_capacity(len);
        res.push(Self::magic_num_0());
        let len_buf = (len as u32).to_be_bytes();
        res.extend_from_slice(&len_buf[..]);
        res.push(Self::magic_num_1());
        let id_buf = mid.to_be_bytes();
        res.extend_from_slice(&id_buf[..]);
        res.push(Self::magic_num_2());
        let ext_buf = ext.to_be_bytes();
        res.extend_from_slice(&ext_buf[..]);
        res.push(tag);
        res.push(Self::magic_num_3());
        res.extend_from_slice(&v[..]);
        res.push(Self::magic_num_4());
        res
    }

    async fn push_cache(&self, id:usize, v:Vec<u8>) -> Result<(), USErr>
    {
        let mut queue = self.queue.lock().await;
        if queue.len() == self.cache_size as usize  {
            let mut msg_cache_queue = self.msg_cache_queue.lock().await;
            msg_cache_queue.push_back((id,v));
            return Err(USErr::MsgCacheOverflow);
            //self.drop_one_cache();
        }
        queue.push_back(id);
        let mut msg_map = self.msg_map.lock().await;
        msg_map.insert(id,(v,SystemTime::now(),1));
        Ok(())
    }

    async fn remove_cache(&self,mid:usize)-> Option<(Vec<u8>,SystemTime,u16)>
    {
        let mut queue = self.queue.lock().await;
        let mut rmi = None;
        for (i,v) in queue.iter().enumerate(){
            if *v == mid
            {
                rmi = Some(i);
                break;
            }
        }
        match rmi {
            None => {
                //eprintln!("remove_cache not found mid {}",mid);
            }
            Some(v) => {
                queue.remove(v);
            }
        }
        let mut msg_map = self.msg_map.lock().await;
        msg_map.remove(&mid)
    }

    async fn unwarp(&mut self,data:&[u8])-> Result<Vec<u8>,USErr>
    {
        let mut d = None;
        if data.is_empty() && self.need_check_in()
        {
            match self.recv_cache.remove(&self.expect_id){
                None => {}
                Some(v) => {
                    self.next_expect();
                    d = Some(v);
                }
            }
        }
        if d.is_none() {
            d = match self.subpacker.subpackage(data, data.len())
            {
                None => { None }
                Some(v) => {
                    let (msg, id, ext,tag) = self.unwarp_ex(v.as_slice());
                    if self.check_send_recv(msg,ext,tag,id).await?
                    {
                        return Err(USErr::EmptyMsg);
                    }
                    //println!("recv msg {} {:?}",id,msg);
                    if id > self.expect_id
                    {
                        self.send_recv(id).await;
                        self.recv_cache.insert(id, (msg.to_vec(), ext,tag));
                        None
                    } else if id == self.expect_id {
                        self.send_recv(id).await;
                        self.next_expect();
                        Some((msg.to_vec(),ext,tag))
                    }else { None }
                }
            };
        }
        if let Some((msg,ext,tag)) = d{
            let mut msg_split = self.msg_split.lock().await;
            if msg_split.need_merge(tag){
                if let Some(v) = msg_split.merge(msg.as_slice(),ext,tag)
                {
                    return Ok(v);
                }
            }else{
                return Ok(msg);
            }
        }
        Err(USErr::EmptyMsg)
    }

    async fn check_send_recv(&mut self,msg:&[u8],ext:u32,tag:u8,id:usize) -> Result<bool,USErr>
    {
        if msg.len() == 1 && msg[0] == 199 && tag == TOKEN_NORMAL {
            if ext == Self::mn_send_recv(){
                //println!("op recv msg {}",id);
                //self.remove_cache(id).await;
                if let Some((_,t,times)) = self.remove_cache(id).await
                {
                    println!("op recv msg id = {} times = {}",id,times);
                    self.check_msg_cache_queue().await;
                }
                // else{
                //     let v = Self::warp_ex(&[199],Self::mn_miss_cache(),TOKEN_NORMAL,id);
                //     Self::send_ex(self.sock.clone(),self.addr,v.as_slice()).await;
                //     return Err(USErr::ResponseSendSuccMiss);
                // }
                Ok(true)
            }else if ext == Self::mn_miss_cache(){
                Err(USErr::SendSuccMissCache)
            }else { Ok(false) }
        }else{
            Ok(false)
        }
    }

    fn next_expect(&mut self)->usize
    {
        if self.expect_id == usize::max_value()
        {
            self.expect_id = usize::one();
        }else{
            self.expect_id += usize::one();
        }
        self.expect_id
    }

    async fn send_recv(&self,id:usize) -> Result<usize,USErr>
    {
        let v = Self::warp_ex(&[199],Self::mn_send_recv(),TOKEN_NORMAL,id);
        self.send(v.as_slice()).await
    }

    async fn send(&self,d:&[u8]) -> Result<usize,USErr>
    {
        let sock = self.sock.clone();
        let addr = self.addr;
        Self::send_ex(sock,addr,d).await
    }

    async fn send_ex(sock:Arc<UdpSocket>,addr:SocketAddr,d:&[u8]) -> Result<usize,USErr>
    {
        match sock.send_to(d,addr).await{
            Ok(l) => {
                Ok(l)
            }
            Err(e) => {
                eprintln!("udp send msg failed {:?}",e);
                Err(USErr::Warp(e))
            }
        }
        //if len != d.len(){  eprintln!("udp send msg failed expect len {} get {}",d.len(),len); }
    }

    fn unwarp_ex<'a>(&self,data: &'a [u8])->(&'a[u8],usize,u32,u8)
    {
        let id_p = size_of::<u8>() + size_of::<u32>();
        let ext_p = id_p + size_of::<usize>() + size_of::<u8>();
        let tag = data[ext_p + size_of::<u32>()];
        let msg_p = ext_p + size_of::<u32>() + size_of::<u8>() * 2;


        let mut id_buf = [0u8;size_of::<usize>()];
        id_buf.copy_from_slice(&data[id_p..(id_p + size_of::<usize>())]);
        let mut ext_buf = [0u8;size_of::<u32>()];
        ext_buf.copy_from_slice(&data[ext_p..(ext_p + size_of::<u32>())]);


        (&data[msg_p..data.len()],usize::from_be_bytes(id_buf),u32::from_be_bytes(ext_buf),tag)
    }

    fn need_check_in(&self)-> bool
    {
        !self.recv_cache.is_empty() && self.recv_cache.contains_key(&self.expect_id)
    }

    fn need_check(&self) -> bool {
        self.subpacker.need_check() || self.need_check_in()
    }

    const fn magic_num_0()->u8 {3}
    const fn magic_num_1()->u8 {1}
    const fn magic_num_2()->u8 {7}
    const fn magic_num_3()->u8 {2}
    const fn magic_num_4()->u8 {6}

    const fn mn_send_recv()->u32 {u32::max_value()}
    const fn mn_miss_cache()->u32 {u32::max_value()-1}

    fn package_len()->usize
    {
        size_of::<u8>() * 4 + size_of::<usize>() + size_of::<u32>() * 2
    }

    fn adjust_unit_size(&mut self,times_frequency:HashMap<u16,u32>)
    {
        // times_frequency.into_iter().for_each(|(times,f)|{
        //     if times > self.max_retry_times / 10 {
        //         self.msg_split.down_unit_size();
        //         println!("down unit size curr = {} ",self.msg_split.unit_size());
        //     }else if times < 3 && f == self.cache_size as u32{
        //         self.msg_split.up_unit_size();
        //         println!("up unit size curr = {} ",self.msg_split.unit_size());
        //     }
        // });
    }

    async fn check_msg_cache_queue(&self)
    {
        let mut queue = self.queue.lock().await;
        let mut msg_cache_queue = self.msg_cache_queue.lock().await;
        loop {
            if queue.len() < self.cache_size as usize
            {
                if let Some((id, v)) = msg_cache_queue.pop_front()
                {
                    let sock = self.sock.clone();
                    let addr = self.addr;
                    self.push_cache(id, v).await.ok().unwrap();
                    let mut msg_map = self.msg_map.lock().await;
                    let (a,_,_) = msg_map.get(&id).unwrap();
                    Self::send_ex(sock, addr, a.as_slice()).await;
                }else { break; }
            }else{break;}
        }
    }

    async fn need_split(&self,len:usize)->bool
    {
        let mut msg_split = self.msg_split.lock().await;
        msg_split.need_split(len)
    }
}

#[async_trait]
impl UdpSender for DefUdpSender
{
    async fn send_msg(&self, v: Vec<u8>) -> Result<(),USErr> {
        if self.need_split(v.len()).await
        {
            let vs = {
                let mut msg_split = self.msg_split.lock().await;
                msg_split.split(&v)
            };
            for (d,ext,tag) in vs
            {
                let v = match self.warp(d,ext,tag).await{
                    Ok(v) => {v}
                    Err(USErr::MsgCacheOverflow) => { continue; }
                    Err(e) => {return Err(e);}
                };
                self.send(v.as_slice()).await?;
            }
            Ok(())
        }else{
            match self.warp(v.as_slice(),0,TOKEN_NORMAL).await{
                Ok(v) => {self.send(v.as_slice()).await?;}
                Err(USErr::MsgCacheOverflow) => {}
                Err(e) => { return Err(e);}
            }
            Ok(())
        }
    }

    async fn check_recv(&mut self, data: &[u8]) -> Result<Vec<u8>,USErr> {
        self.unwarp(data).await
    }


    fn need_check(&self) -> bool {
        self.need_check()
    }

    fn create(sock: Arc<UdpSocket>,addr:SocketAddr) -> Self {
        let cache_size = 50;
        let max_len = 65500 - Self::package_len();
        let min_len = 1500 - Self::package_len();
        DefUdpSender{
            addr,
            sock,
            max_len: max_len as _,
            min_len: min_len as _,
            cache_size,
            mid: Arc::new(Mutex::new(usize::zero())),
            queue: Arc::new(Mutex::new(VecDeque::new())),
            msg_map: Arc::new(Mutex::new(HashMap::new())),
            expect_id: 1,
            recv_cache: HashMap::new(),
            subpacker: UdpSubpackage::new(),
            timeout: Duration::from_millis(40),
            msg_split: Arc::new(Mutex::new(UdpMsgSplit::with_max_unit_size(max_len,min_len))),
            max_retry_times: 100,
            msg_cache_queue: Arc::new(Mutex::new(VecDeque::new()))
        }
    }

    async fn set_max_msg_len(&mut self, len: u16) {
        self.max_len = len;
        let mut msg_split = self.msg_split.lock().await;
        msg_split.set_max_unit_size(len as _);
    }

    fn max_msg_len(&self) -> u16 {
        self.max_len
    }

    async fn set_min_msg_len(&mut self, len: u16) {
        self.min_len = len;
        let mut msg_split = self.msg_split.lock().await;
        msg_split.set_min_unit_size(len as _)
    }

    fn min_msg_len(&self) -> u16 {
        self.min_len
    }

    fn cache_size(&self) -> u16 {
        self.cache_size
    }

    fn set_cache_size(&mut self, s: u16) {
        self.cache_size = s;
    }

    fn set_time_out(&mut self, dur: Duration) {
        self.timeout = dur;
    }

    async fn check_send(&mut self) -> Result<(),USErr> {
        let now = SystemTime::now();
        let mut times_frequency = HashMap::<u16,u32>::new();
        {
            let mut msg_map = self.msg_map.lock().await;
            for (id, (v, t, times)) in msg_map.iter_mut() {
                if *times > self.max_retry_times {
                    eprintln!("msg {} retry times reach the maximum!", id);
                    return Err(USErr::RetryTimesLimit);
                }
                if let Ok(dur) = now.duration_since(*t)
                {
                    if dur > self.timeout
                    {
                        *times += 1;
                        *t = SystemTime::now();
                        Self::send_ex(self.sock.clone(), self.addr, v.as_slice()).await;
                    }
                }
                let mut freq = *(times_frequency.get(times).unwrap_or(&0));
                freq += 1;
                times_frequency.insert(*times, freq);
            }
        }

        self.adjust_unit_size(times_frequency);
        //self.check_msg_cache_queue().await;
        Ok(())
    }

    fn set_retry_times(&mut self, v: u16) {
        self.max_retry_times = v;
    }

    fn retry_times(&self) -> u16 {
        self.max_retry_times
    }
}




