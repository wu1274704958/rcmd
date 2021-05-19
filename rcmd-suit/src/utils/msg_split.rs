use crate::agreement::Message;
use std::collections::{HashMap, HashSet, VecDeque};
use std::mem::size_of;
use crate::tools::{TOKEN_SUBPACKAGE, TOKEN_SUBPACKAGE_END, TOKEN_SUBPACKAGE_BEGIN, TOKEN_NORMAL};
use crate::ext_code::*;
use std::time::SystemTime;
use chrono::prelude::*;
use mysql::chrono::Local;

pub trait MsgSplit{
    fn open(&self) ->bool { false }
    fn max_unit_size(&self)-> usize {1024}
    fn need_split(&self,len:usize,ext:u32) ->bool {
        self.open() && len > self.max_unit_size() && !self.ignore(ext)
    }
    fn ignore(&self,ext:u32)-> bool;
    fn split<'a>(&mut self,data:&'a mut Vec<u8>,ext:u32) -> Vec<(&'a [u8],u32,u8)>;
    fn need_merge<'a>(&self,msg:&'a Message<'a>)->bool;
    fn merge<'a>(&mut self,msg:&'a Message<'a>)->Option<(Vec<u8>, u32)>;
    fn extend_ignore(&mut self,v:&[u32]);
}

pub trait UdpMsgSplit{
    fn with_max_unit_size(max_unit_size:usize,min_unit_size:usize)->Self;

    fn is_max_unit_size(&self)->bool;

    fn is_min_unit_size(&self)->bool;

    fn up_unit_size(&mut self);

    fn down_unit_size(&mut self);

    fn need_split(&self,len:usize) ->bool;

    fn open(&self) -> bool;

    fn max_unit_size(&self) -> usize;

    fn need_merge<'a>(&self, tag:u8) -> bool;

    fn merge<'a>(&mut self, msg:&[u8],ext:u32,tag:u8,sub_head:&[u8]) -> Option<Vec<u8>>;

    fn set_max_unit_size(&mut self, max_unit_size: usize);
    fn set_min_unit_size(&mut self, min_unit_size: usize);
    fn min_unit_size(&self) -> usize;
    fn unit_size(&self) -> usize;
    fn need_send(&self) ->bool;

    fn push_msg(&mut self,v:Vec<u8>);

    fn pop_msg(&mut self) -> Option<(&[u8], u32, u8,Option<Vec<u8>>)>;
}

pub struct DefMsgSplit{
    msg_cache:HashMap<u16,(Vec<u8>,u16)>,
    ignore_map: HashSet<u32>,
    logic_id:u16
}

impl DefMsgSplit{
    pub fn new()->DefMsgSplit{
        let mut ignore_map = HashSet::new();
        ignore_map.extend([9].iter());
        DefMsgSplit{
            msg_cache:HashMap::new(),
            logic_id:0,
            ignore_map
        }
    }

    pub fn get_id(&mut self)->u16
    {
        if self.logic_id == u16::max_value(){
            self.logic_id = 0
        }
        self.logic_id += 1;
        self.logic_id
    }

    pub fn parse_ext(ext:u32)->(u16,u16)
    {
        let buf = ext.to_be_bytes();
        let mut f = [0u8;size_of::<u16>()];
        let mut s = [0u8;size_of::<u16>()];
        f.copy_from_slice(&buf[0..size_of::<u16>()]);
        s.copy_from_slice(&buf[size_of::<u16>()..]);
        (u16::from_be_bytes(f),u16::from_be_bytes(s))
    }

}

impl MsgSplit for DefMsgSplit
{
    fn open(&self) -> bool {
        true
    }

    fn max_unit_size(&self) -> usize {
        501
    }

    fn ignore(&self, ext: u32) -> bool {
        self.ignore_map.contains(&ext)
    }

    fn split<'a>(&mut self,data:&'a mut Vec<u8>,ext:u32) -> Vec<(&'a [u8],u32,u8)> {
        let mut b = 0usize;
        let l = self.max_unit_size();
        let mut res = Vec::new();
        let id = self.get_id();
        let id_ = id.to_be_bytes();
        let mut i = 0u16;
        let len = data.len();
        data.extend_from_slice(&id_);
        loop{
            if b >= len { break; }
            let e = if b + l <= len { b + l } else { len };
            let sli = &data[b..e];
            let begin = b == 0;

            let mut ext_buf = [0u8;size_of::<u32>()];
            (&mut ext_buf[0..size_of::<u16>()]).copy_from_slice(&id_);
            let i_ = i.to_be_bytes();
            (&mut ext_buf[size_of::<u16>()..]).copy_from_slice(&i_);

            let tag = if begin{
                TOKEN_SUBPACKAGE_BEGIN
            }else{
                TOKEN_SUBPACKAGE
            };

            res.push((sli,u32::from_be_bytes(ext_buf),tag));

            i += 1;
            b = e;
        }
        res.push((&data[len..],ext,TOKEN_SUBPACKAGE_END));
        res
    }

    fn need_merge<'a>(&self, msg: &'a Message<'a>) -> bool {
        msg.tag >= TOKEN_SUBPACKAGE_BEGIN && msg.tag <= TOKEN_SUBPACKAGE_END
    }

    fn merge<'a>(&mut self, msg: &'a Message<'a>) -> Option<(Vec<u8>, u32)> {
        match msg.tag {
            TOKEN_SUBPACKAGE_BEGIN => {
                let (id,idx) = Self::parse_ext(msg.ext);
                assert_eq!(idx,0);
                if self.msg_cache.contains_key(&id)
                {
                    eprintln!("Has not ended subpackage will drop!!!");
                    self.msg_cache.remove(&id);
                }
                self.msg_cache.insert(id,(msg.msg.to_vec(),idx));
                None
            }
            TOKEN_SUBPACKAGE => {
                let (id,idx) = Self::parse_ext(msg.ext);
                if self.msg_cache.contains_key(&id)
                {
                    let (cache,idx_) = self.msg_cache.get_mut(&id).unwrap();
                    assert_eq!(*idx_+1,idx);
                    cache.reserve(msg.msg.len());
                    cache.extend_from_slice(&msg.msg);
                    *idx_ = idx;
                }else{
                    eprintln!("Not found this subpackage id!!!");
                }
                None
            }
            TOKEN_SUBPACKAGE_END => {
                assert_eq!(msg.msg.len(),size_of::<u16>());
                let mut buf = [0u8;size_of::<u16>()];
                buf.copy_from_slice(&msg.msg[..]);
                let id = u16::from_be_bytes(buf);
                if self.msg_cache.contains_key(&id)
                {
                    let (cache,idx_) = self.msg_cache.remove(&id).unwrap();
                    return Some((cache,msg.ext));
                }else{
                    eprintln!("Not found this subpackage id!!!");
                    None
                }
            }
            _ => {None}
        }
    }

    fn extend_ignore(&mut self, v: &[u32]) {
        self.ignore_map.extend(v.iter());
    }
}

pub struct DefUdpMsgSplit
{
    msg_cache:HashMap<u32,(Vec<u8>,u32)>,
    logic_id:u32,
    max_unit_size:usize,
    min_unit_size:usize,
    unit_size:usize,
    curr_idx:Option<usize>,
    wait_split_queue:VecDeque<(Vec<u8>,usize,u32)>,
    recovery_info : VecDeque<(u32,u32,u32)>,
    cache_size:usize
}

impl DefUdpMsgSplit{

    pub fn get_id(&mut self)->u32
    {
        if self.logic_id == u32::max_value(){
            self.logic_id = 0
        }
        self.logic_id += 1;
        self.logic_id
    }

    fn check_wait_split_queue(&mut self)
    {
        while self.wait_split_queue.len() > self.cache_size {
            let using = if let Some(v) = self.curr_idx{
                v == 0
            }else{
                false
            };
            if !using {
                let v = self.wait_split_queue.pop_front().unwrap();
                while let Some(info) = self.recovery_info.front() {
                    if (*info).0 == v.2{
                        drop(info);
                        self.recovery_info.pop_front();
                    }else { break; }
                }
                if let Some(v) = self.curr_idx{
                    self.curr_idx = Some(v - 1);
                }
            }else{
                break;
            }
        }
    }

    fn move_next_msg(&mut self)
    {
        let mut v = self.curr_idx.unwrap();
        if v + 1 < self.wait_split_queue.len()
        {
            self.curr_idx = Some(v + 1);
        }else{
            self.curr_idx = None;
        }
    }
}

impl UdpMsgSplit for DefUdpMsgSplit {
    fn with_max_unit_size(max_unit_size: usize, min_unit_size: usize) -> Self {
        DefUdpMsgSplit{
            msg_cache:HashMap::new(),
            logic_id:0,
            max_unit_size,
            min_unit_size,
            unit_size:max_unit_size,
            wait_split_queue:VecDeque::new(),
            curr_idx: None,
            cache_size: 3,
            recovery_info : VecDeque::new()
        }
    }

    fn is_max_unit_size(&self)->bool
    {
        self.max_unit_size == self.unit_size
    }

    fn is_min_unit_size(&self)->bool
    {
        self.min_unit_size == self.unit_size
    }

    fn up_unit_size(&mut self)
    {
        let mut n = self.unit_size + (self.unit_size / 10);
        if n > self.max_unit_size { n = self.max_unit_size; }
        self.unit_size = n;
    }

    fn down_unit_size(&mut self)
    {
        let mut n = self.unit_size - (self.unit_size / 30);
        if n < self.min_unit_size { n = self.min_unit_size; }
        self.unit_size = n;
    }

    fn need_split(&self,len:usize) ->bool {
        (self.open() && len > self.max_unit_size()) || !self.wait_split_queue.is_empty()
    }

    fn open(&self) -> bool {
        true
    }

    fn max_unit_size(&self) -> usize {
        self.unit_size
    }

    fn need_merge<'a>(&self, tag:u8) -> bool {
        tag >= TOKEN_SUBPACKAGE_BEGIN && tag <= TOKEN_SUBPACKAGE_END
    }

    fn merge<'a>(&mut self, msg:&[u8],ext:u32,tag:u8,sub_head:&[u8]) -> Option<Vec<u8>> {
        dbg!(sub_head);
        // match tag {
        //     TOKEN_SUBPACKAGE_BEGIN => {
        //         let (id,idx) = Self::parse_ext(ext);
        //         assert_eq!(idx,0);
        //         if self.msg_cache.contains_key(&id)
        //         {
        //             eprintln!("Has not ended subpackage will drop!!!");
        //             self.msg_cache.remove(&id);
        //         }
        //         self.msg_cache.insert(id,(msg.to_vec(),idx));
        //         None
        //     }
        //     TOKEN_SUBPACKAGE => {
        //         let (id,idx) = Self::parse_ext(ext);
        //         if self.msg_cache.contains_key(&id)
        //         {
        //             let (cache,idx_) = self.msg_cache.get_mut(&id).unwrap();
        //             assert_eq!(*idx_+1,idx);
        //             cache.reserve(msg.len());
        //             cache.extend_from_slice(&msg[..]);
        //             *idx_ = idx;
        //         }else{
        //             eprintln!("Not found this subpackage id!!!");
        //         }
        //         None
        //     }
        //     TOKEN_SUBPACKAGE_END => {
        //
        //         let (id,idx) = Self::parse_ext(ext);
        //         if self.msg_cache.contains_key(&id)
        //         {
        //             let (mut cache,idx_) = self.msg_cache.remove(&id).unwrap();
        //             assert_eq!(idx_+1,idx);
        //             cache.reserve(msg.len());
        //             cache.extend_from_slice(&msg[..]);
        //             return Some(cache);
        //         }else{
        //             eprintln!("Not found this subpackage id!!!");
        //         }
        //         None
        //     }
        //     _ => {None}
        // }
        None
    }
    fn set_max_unit_size(&mut self, max_unit_size: usize) {
        self.max_unit_size = max_unit_size;
    }
    fn set_min_unit_size(&mut self, min_unit_size: usize) {
        self.min_unit_size = min_unit_size;
    }
    fn min_unit_size(&self) -> usize {
        self.min_unit_size
    }
    fn unit_size(&self) -> usize {
        self.unit_size
    }

    fn need_send(&self) ->bool
    {
        !self.wait_split_queue.is_empty()
    }

    fn push_msg(&mut self,v:Vec<u8>)
    {
        self.check_wait_split_queue();
        let id = self.get_id();
        self.wait_split_queue.push_back((v,0,id));
        if let None = self.curr_idx
        {
            self.curr_idx = Some(0);
        }
    }

    fn pop_msg(&mut self) -> Option<(&[u8], u32, u8,Option<Vec<u8>>)>
    {
        let curr_idx = if let Some(v) = self.curr_idx {
            v
        }else{
            return None;
        };
        let mut recovery_info = None;
        let mut move_next = false;
        let wait_split_queue_len = self.wait_split_queue.len();
        //(data begin_pos id )
        let res = if let Some(v) = self.wait_split_queue.get_mut(curr_idx)
        {
            let end = (*v).0.len() - (*v).1 <= self.unit_size;
            let e = if end { (*v).0.len()  } else{ (*v).1 + self.unit_size};
            let sli =  &(*v).0[(*v).1..e] ;
            let begin = (*v).1 == 0;
            if begin && end {
                move_next = true;
                recovery_info = Some(((*v).2,0,e as u32));
                Some((sli,0,TOKEN_NORMAL,None))
            }else {
                let ticks = Local::now().timestamp_millis();

                let mut sub_head = Vec::with_capacity(size_of::<i64>() + size_of::<u32>() * 2);
                sub_head.extend_from_slice(&ticks.to_be_bytes());
                sub_head.extend_from_slice(&((*v).1 as u32).to_be_bytes()); //Begin pos
                sub_head.extend_from_slice(&((*v).0.len() as u32).to_be_bytes());//Message length

                let ext = (*v).2;

                (*v).1 = e;

                recovery_info = Some(((*v).2, (*v).1 as u32, e as u32));

                let tag = if begin {
                    TOKEN_SUBPACKAGE_BEGIN
                } else if end {
                    TOKEN_SUBPACKAGE_END
                } else { TOKEN_SUBPACKAGE };

                if end { move_next = true; }
                Some((sli, ext, tag, Some(sub_head)))
            }
        }else { return None; };
        if let Some(v) = recovery_info{
            self.recovery_info.push_back(v);
        }
        if move_next {
            let mut v = self.curr_idx.unwrap();
            if v + 1 < wait_split_queue_len
            {
                self.curr_idx = Some(v + 1);
            }else{
                self.curr_idx = None;
            }
        }
        res
    }
}
