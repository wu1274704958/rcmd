use crate::agreement::Message;
use std::collections::{HashMap, HashSet, VecDeque};
use std::mem::size_of;
use crate::tools::{TOKEN_SUBPACKAGE, TOKEN_SUBPACKAGE_END, TOKEN_SUBPACKAGE_BEGIN, TOKEN_NORMAL};
use crate::ext_code::*;
use std::time::SystemTime;
use crate::utils::stream_parser::{Stream,StreamParse};
use std::process::abort;

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

    fn pop_msg(&mut self) -> Option<(&[u8], u32, u8,MsgSlicesInfo)>;

    fn recovery(&mut self,id:u32)->bool;
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

pub enum MsgSlicesInfo {
    Complete,
    Part(Vec<u8>),
}

pub struct DefUdpMsgSplit
{
    msg_cache:HashMap<u32,(Vec<u8>,u32,u128)>,
    last_pop_logic_id:Option<u32>,
    logic_id:u32,
    max_unit_size:usize,
    min_unit_size:usize,
    unit_size:usize,
    curr_idx:Option<usize>,
    ///data begin_pos lid sub_msg_idx
    wait_split_queue:VecDeque<(Vec<u8>,usize,u32,u32)>,
    ///lid begin_pos idx sub_msg_idx
    recovery_info : VecDeque<(u32,u32,u32,u32)>,
    cache_size:usize,
    next_idx:Option<usize>
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

    fn remove_front(&mut self) -> bool
    {
        if let Some(d) = self.wait_split_queue.pop_front()
        {
            loop{
                if let Some(r) = self.recovery_info.front()
                {
                    if (*r).0 != d.2{ break; }
                }else { break; }
                self.recovery_info.pop_front();
            }
            return true;
        }else{
            return false;
        }
    }

    fn check_wait_split_queue(&mut self)
    {
        if let Some(curr) = self.curr_idx{
            let offset = -(self.cache_size as isize);
            let top = curr as isize + offset;
            if top < 0{ return; }
            let new_top = top as usize;
            for _i in 0..new_top{
                self.remove_front();
            }
            self.curr_idx = Some(curr - new_top);
            for r in self.recovery_info.iter_mut(){
                (*r).2 -= new_top as u32;
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

    fn greater_than(a:u32,b:u32)->bool{
        const R:u32 = 1000;
        if (a > u32::max_value() - R && a <= u32::max_value()) && (b >= 0 && b < 1000) ||
            (b > u32::max_value() - R && b <= u32::max_value()) && (a >= 0 && a < 1000)
        {
            return a < b;
        }
        a > b
    }
}

#[test]
fn test_greater_than()
{
    assert_eq!(DefUdpMsgSplit::greater_than(0,2),false);
    assert_eq!(DefUdpMsgSplit::greater_than(3,5),false);
    assert_eq!(DefUdpMsgSplit::greater_than(6,288),false);
    assert_eq!(DefUdpMsgSplit::greater_than(0,1000000),false);
    assert_eq!(DefUdpMsgSplit::greater_than(1000000,0),true);
    assert_eq!(DefUdpMsgSplit::greater_than(u32::max_value() - 89,2),false);
    assert_eq!(DefUdpMsgSplit::greater_than(u32::max_value(),2),false);
    assert_eq!(DefUdpMsgSplit::greater_than(u32::max_value() - 89,0),false);
    assert_eq!(DefUdpMsgSplit::greater_than(0 ,u32::max_value() - 999),true);
    assert_eq!(DefUdpMsgSplit::greater_than(999 ,u32::max_value() - 999),true);
    assert_eq!(DefUdpMsgSplit::greater_than(999 ,u32::max_value()),true);
    assert_eq!(DefUdpMsgSplit::greater_than(u32::max_value() - 999,0 ),false);
    assert_eq!(DefUdpMsgSplit::greater_than(u32::max_value() - 999,999 ),false);
    assert_eq!(DefUdpMsgSplit::greater_than(u32::max_value(),999 ),false);
}

impl UdpMsgSplit for DefUdpMsgSplit {
    fn with_max_unit_size(max_unit_size: usize, min_unit_size: usize) -> Self {
        DefUdpMsgSplit{
            msg_cache:HashMap::new(),
            logic_id:0,
            max_unit_size,
            min_unit_size,
            unit_size:(max_unit_size+min_unit_size)/2,
            wait_split_queue:VecDeque::new(),
            curr_idx: None,
            cache_size: 10,
            recovery_info : VecDeque::new(),
            next_idx :None,
            last_pop_logic_id : None
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
        let mut n = self.unit_size - (self.unit_size / 2);
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
        if let Some(last_lid) = self.last_pop_logic_id{
            if last_lid == ext || Self::greater_than(last_lid,ext){  return None;  }
        }
        //dbg!(sub_head);
        let mut stream = Stream::new(sub_head);
        let ticks = u128::stream_parse(&mut stream).unwrap();
        let begin_pos = u32::stream_parse(&mut stream).unwrap() as usize;
        let msg_len = u32::stream_parse(&mut stream).unwrap() as usize;
        let sub_idx = u32::stream_parse(&mut stream).unwrap();
        if msg_len == 0 { return None; }
        //println!("{:?} b {:?} tick {:?} data len {:?} ",sub_idx,begin_pos,ticks,msg_len);
        match tag {
            TOKEN_SUBPACKAGE_BEGIN => {
                if self.msg_cache.contains_key(&ext)
                {
                    if let Some(d) = self.msg_cache.get_mut(&ext){
                        if (*d).0.len() != msg_len { return None; }
                        if ticks > (*d).2
                        {
                            (&mut (*d).0[begin_pos..(begin_pos + msg.len())]).copy_from_slice(msg);
                            (*d).1 = (begin_pos + msg.len()) as u32;
                            (*d).2 = ticks;
                        }
                    }
                }else{
                    let mut d = Vec::with_capacity(msg_len);
                    d.resize(msg_len,0u8);
                    (&mut d[begin_pos..(begin_pos + msg.len())]).copy_from_slice(msg);
                    self.msg_cache.insert(ext,(d,(begin_pos + msg.len()) as u32,ticks));
                }
            }
            TOKEN_SUBPACKAGE => {
                if let Some(d) = self.msg_cache.get_mut(&ext){
                    if (*d).0.len() != msg_len { return None; }
                    if ticks > (*d).2
                    {
                        (&mut (*d).0[begin_pos..(begin_pos + msg.len())]).copy_from_slice(msg);
                        (*d).1 = (begin_pos + msg.len()) as u32;
                        (*d).2 = ticks;
                    }
                }
            }
            TOKEN_SUBPACKAGE_END => {
                if begin_pos + msg.len() != msg_len  { return None; }
                let mut pop = false;
                if let Some(d) = self.msg_cache.get_mut(&ext){
                    if (*d).0.len() != msg_len {return None;}
                    if ticks > (*d).2
                    {
                        (&mut (*d).0[begin_pos..(begin_pos + msg.len())]).copy_from_slice(msg);
                        (*d).1 = (begin_pos + msg.len()) as u32;
                        (*d).2 = ticks;
                        pop = true;
                    }
                }
                if pop {
                    if let Some(d) = self.msg_cache.remove(&ext)
                    {
                        self.last_pop_logic_id = Some(ext);
                        return Some(d.0);
                    }
                }
            }
             _ => {}
        }
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
        self.curr_idx.is_some() && !self.wait_split_queue.is_empty()
    }

    fn push_msg(&mut self,v:Vec<u8>)
    {
        self.check_wait_split_queue();
        let id = self.get_id();
        self.wait_split_queue.push_back((v,0,id,0));
        if let None = self.curr_idx
        {
            self.curr_idx = Some(self.wait_split_queue.len() - 1);
        }
    }

    fn pop_msg(&mut self) -> Option<(&[u8], u32, u8,MsgSlicesInfo)>
    {
        let curr_idx = if let Some(v) = self.curr_idx {
            v
        }else{
            return None;
        };
        let mut recovery_info = None;
        let mut move_next = false;
        let wait_split_queue_len = self.wait_split_queue.len();
        ///data begin_pos lid sub_msg_idx
        let res = if let Some(v) = self.wait_split_queue.get_mut(curr_idx)
        {
            let mut end = (*v).0.len() - (*v).1 <= self.unit_size;
            let mut e = if end { (*v).0.len()  } else{ (*v).1 + self.unit_size};
            let mut sli =  &(*v).0[(*v).1..e] ;
            let begin = (*v).1 == 0;

            if (begin && end) && sli.len() > self.min_unit_size {
                end = false;
            }

            if begin && end {
                move_next = true;
                recovery_info = None;//Some(((*v).2,0,curr_idx as u32));
                if (*v).1 == 0{
                    (*v).1 = e;
                    Some((sli,0,TOKEN_NORMAL,MsgSlicesInfo::Complete))
                }else{
                    None
                }
            }else {
                let ticks = SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();

                let mut sub_head = Vec::with_capacity(size_of::<u128>() + size_of::<u32>() * 2);
                sub_head.extend_from_slice(&ticks.to_be_bytes());
                sub_head.extend_from_slice(&((*v).1 as u32).to_be_bytes()); //Begin pos
                sub_head.extend_from_slice(&((*v).0.len() as u32).to_be_bytes());//Message length
                sub_head.extend_from_slice(&(*v).3.to_be_bytes());//sub_msg_idx

                let ext = (*v).2;
                //lid begin_pos idx sub_msg_idx
                recovery_info = Some(((*v).2, (*v).1 as u32, curr_idx as u32,(*v).3));

                (*v).1 = e;
                (*v).3 += 1;
                let tag = if begin {
                    TOKEN_SUBPACKAGE_BEGIN
                } else if end {
                    TOKEN_SUBPACKAGE_END
                } else { TOKEN_SUBPACKAGE };

                if end { move_next = true; }
                Some((sli, ext, tag, MsgSlicesInfo::Part(sub_head)))
            }
        }else { return None; };
        if let Some(v) = recovery_info{
            self.recovery_info.push_back(v);
        }
        if move_next {
            let mut v = self.curr_idx.unwrap();
            if self.next_idx.is_some(){
                self.curr_idx = self.next_idx;
                self.next_idx = None;
            }else if v + 1 < wait_split_queue_len
            {
                self.curr_idx = Some(v + 1);
            }else{
                self.curr_idx = None;
            }
        }
        res
    }

    fn recovery(&mut self, id: u32) -> bool {
        ///lid begin_pos idx sub_msg_idx
        if let Some(v) = self.recovery_info.back(){
            //dbg!(v);
            if (*v).0 != id {
                return false;
            }
            if let Some(idx) = self.curr_idx{
                if (*v).2 as usize > idx
                {
                    //println!("(*v).2 as usize > idx !!!!!!!!{} {}",(*v).2,idx);
                    return false;
                }
            }
        }else{
            //println!("recovery_info no Back");
            return false;
        }
        let info = self.recovery_info.pop_back().unwrap();
        //println!("reco {:?}",info);
        self.curr_idx = Some(info.2 as usize);
        ///data begin_pos lid sub_msg_idx
        if let Some(msg) = self.wait_split_queue.get_mut(info.2 as usize)
        {
            (*msg).1 = info.1 as usize;
            (*msg).3 = info.3;
        }else{
            println!("Not find recovery message!");
            abort();
        }
        true
    }
}
