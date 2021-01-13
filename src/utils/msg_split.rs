use crate::agreement::Message;
use std::collections::HashMap;
use std::mem::size_of;
use crate::tools::{TOKEN_SUBPACKAGE, TOKEN_SUBPACKAGE_END, TOKEN_SUBPACKAGE_BEGIN};

pub trait MsgSplit{
    fn open(&self) ->bool { false }
    fn max_unit_size(&self)-> usize {1024}
    fn need_split(&self,len:usize) ->bool {
        self.open() && len > self.max_unit_size()
    }
    fn split<'a>(&mut self,data:&'a mut Vec<u8>,ext:u32) -> Vec<(&'a [u8],u32,u8)>;
    fn need_merge<'a>(&self,msg:&'a Message<'a>)->bool;
    fn merge<'a>(&mut self,msg:&'a Message<'a>)->Option<(Vec<u8>, u32)>;
}

pub struct DefMsgSplit{
    msg_cache:HashMap<u16,(Vec<u8>,u16)>,
    logic_id:u16
}

impl DefMsgSplit{
    pub fn new()->DefMsgSplit{
        DefMsgSplit{
            msg_cache:HashMap::new(),
            logic_id:0
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
}