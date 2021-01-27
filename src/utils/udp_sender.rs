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

#[async_trait]
pub trait UdpSender{
    async fn send(&self,v:Vec<u8>);
    async fn check_recv(&self,data:&[u8],len:usize)-> Option<Vec<u8>>;
    fn need_check(&self)->bool;
    fn create(sock:Arc<UdpSocket>,addr:SocketAddr) ->Self;
    fn set_max_msg_len(&mut self,len:u16);
    fn max_msg_len(&self)->u16;
    fn set_min_msg_len(&mut self,len:u16);
    fn min_msg_len(&self)->u16;
    fn cache_size(&self)->u16;
    fn set_cache_size(&mut self,s:u16);
    fn set_time_out(&mut self,dur:Duration);
    async fn check_send(&mut self);
}

pub struct DefUdpSender{
    sock: Arc<UdpSocket>,
    max_len: u16,
    min_len: u16,
    cache_size:u16,
    mid: usize,
    queue: VecDeque<usize>,
    msg_map: HashMap<usize,Vec<u8>>,
    recv_cache: HashMap<usize,(Vec<u8>,u32,u8)>,
    expect_id: usize,
    addr:SocketAddr,
    subpacker: UdpSubpackage,
    timeout: Duration,
    timeout_map: HashMap<usize,SystemTime>,
    msg_split: UdpMsgSplit
}

impl DefUdpSender
{
    fn get_mid(&mut self)->usize
    {
        if self.mid == usize::max_value()
        {
            self.mid = usize::one();
        }else{
            self.mid += usize::one();
        }
        self.mid
    }

    fn warp(&mut self,v:&[u8],ext:u32,tag:u8)->Vec<u8>
    {
        let mid = self.get_mid();
        let res = Self::warp_ex(v,ext,tag,mid);
        self.push_cache(mid,res.clone());
        res
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

    fn drop_one_cache(&mut self)
    {
        if let Some(id) = self.queue.pop_front()
        {
            self.msg_map.remove(&id).unwrap();
        }
    }

    fn push_cache(&mut self,id:usize,v:Vec<u8>)
    {
        if self.queue.len() == self.cache_size as usize  {
            self.drop_one_cache();
        }
        self.queue.push_back(id);
        self.msg_map.insert(id,v);
    }

    fn get_cache(&self,mid:usize)->Option<Vec<u8>>
    {
        if let Some(v) = self.msg_map.get(&mid)
        {
            Some(v.clone())
        }else{
            None
        }
    }

    fn unwarp(&mut self,data:&[u8])-> Option<Vec<u8>>
    {
        let mut d = None;
        if self.need_check_in()
        {
            match self.recv_cache.remove(&self.expect_id){
                None => {}
                Some(v) => {
                    self.expect_id += 1;
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
                    self.send_recv(id);
                    if id > self.expect_id
                    {
                        self.recv_cache.insert(id, (msg.to_vec(), ext,tag));
                        None
                    } else if id == self.expect_id {
                        Some((msg.to_vec(),ext,tag))
                    }else { None }
                }
            };
        }
        if let Some((msg,ext,tag)) = d{

        }
        None
    }

    async fn send_recv(&self,id:usize)
    {
        let v = Self::warp_ex(&[199],Self::mn_send_recv(),TOKEN_NORMAL,id);
        self.send(v.as_slice());
    }

    async fn send(&self,d:&[u8])
    {
        let len = match self.sock.send_to(d,self.addr).await{
            Ok(l) => {
                l
            }
            Err(e) => {
                eprintln!("udp send msg failed {:?}",e);
                0
            }
        };
        if len != d.len(){  eprintln!("udp send msg failed expect len {} get {}",d.len(),len); }
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


        (&data[msg_p..(data.len()-1)],usize::from_be_bytes(id_buf),u32::from_be_bytes(ext_buf),tag)
    }

    fn need_check_in(&self)-> bool
    {
        !self.recv_cache.is_empty() && self.recv_cache.contains_key(&self.expect_id)
    }

    fn need_check(&self) -> bool {
        self.subpacker.need_check() || self.need_check_in()
    }

    fn magic_num_0()->u8 {3}
    fn magic_num_1()->u8 {1}
    fn magic_num_2()->u8 {7}
    fn magic_num_3()->u8 {2}
    fn magic_num_4()->u8 {6}

    fn mn_send_recv()->u32 {u32::max_value()}

    fn package_len()->usize
    {
        size_of::<u8>() * 4 + size_of::<usize>() + size_of::<u32>() * 2
    }
}

#[async_trait]
impl UdpSender for DefUdpSender
{
    async fn send(&self, v: Vec<u8>) {

    }

    async fn check_recv(&self, data: &[u8], len: usize) -> Option<Vec<u8>> {
        unimplemented!()
    }


    fn need_check(&self) -> bool {
        unimplemented!()
    }

    fn create(sock: Arc<UdpSocket>,addr:SocketAddr) -> Self {
        let cache_size = 10;
        let max_len = 65500 - Self::package_len();
        let min_len = 1500 - Self::package_len();
        DefUdpSender{
            addr,
            sock,
            max_len: max_len as _,
            min_len: min_len as _,
            cache_size,
            mid: usize::zero(),
            queue: VecDeque::new(),
            msg_map: HashMap::new(),
            expect_id: 1,
            recv_cache: HashMap::new(),
            subpacker: UdpSubpackage::new(),
            timeout: Duration::from_millis(10),
            timeout_map: HashMap::new(),
            msg_split: UdpMsgSplit::with_max_unit_size(max_len,min_len)
        }
    }

    fn set_max_msg_len(&mut self, len: u16) {
        self.max_len = len;
        self.msg_split.set_max_unit_size(len as _);
    }

    fn max_msg_len(&self) -> u16 {
        self.max_len
    }

    fn set_min_msg_len(&mut self, len: u16) {
        self.min_len = len;
        self.msg_split.set_min_unit_size(len as _)
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

    async fn check_send(&mut self) {
        unimplemented!()
    }
}



