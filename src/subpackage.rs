use crate::subpackage::SpState::ExpectBegin;
use crate::tools::{TOKEN_BEGIN, u32_form_bytes, TOKEN_END, TOKEN_MID, TOKEN_NORMAL, TOKEN_SUBPACKAGE, TOKEN_SUBPACKAGE_END, TOKEN_SUBPACKAGE_BEGIN};
use std::mem::size_of;
use std::time::SystemTime;

pub trait Subpackage{
    fn subpackage(&mut self,data:&[u8],len:usize) ->Option<Vec<u8>>
    {
        None
    }
    fn need_check(&self)->bool
    {
        false
    }
}

pub enum SpState {
    ExpectBegin,
    ExpectLen,
    Content,
    ExpectMid,
    ExpectExt,
    ExpectEnd
}

pub struct DefSubpackage {
    temp:Vec<u8>,
    st:SpState,
    bp:Option<usize>,
    idx:usize,
    len:Option<u32>,
    ext:Option<u32>,
    need_ck:bool
}

impl DefSubpackage{
    pub fn new()->DefSubpackage
    {
        DefSubpackage{
            temp:Vec::new(),
            st:ExpectBegin,
            bp:None,
            idx:0,
            len:None,
            ext:None,
            need_ck:false
        }
    }

    pub fn good_sign(b:u8) -> bool
    {
        b == TOKEN_NORMAL || b == TOKEN_SUBPACKAGE || b == TOKEN_SUBPACKAGE_END || b == TOKEN_SUBPACKAGE_BEGIN
    }
}

impl Subpackage for DefSubpackage
{
    fn subpackage(&mut self,data: &[u8], len: usize) -> Option<Vec<u8>> {
        self.need_ck = false;
        if data.len() != 0 {
            self.temp.reserve(data.len());
            for i in 0..len { self.temp.push(data[i]); }
        }
        if self.temp.is_empty() { return None; }
        'Out: loop {
            match self.st {
                SpState::ExpectBegin => {
                    assert_eq!(self.idx, 0, "ExpectBegin idx must be eq 0!");
                    while !self.temp.is_empty() {
                        if self.temp[self.idx] == TOKEN_BEGIN {
                            if self.temp.len() < size_of::<u8>() + size_of::<u32>() + size_of::<u8>() { return None; }
                            let len = u32_form_bytes(&self.temp[(self.idx + size_of::<u8>())..]);
                            if !Self::good_sign(self.temp[self.idx + size_of::<u8>() + size_of::<u32>()]){
                                println!("{:?} i={} len={} bad package not found good TOKEN_SIGN!",self.temp,self.idx,len);
                                self.temp.pop();
                                return None;
                            }
                            if self.temp.len() < size_of::<u8>() * 2 + len as usize { return None; }
                            if self.temp[self.idx + len as usize + 1] == TOKEN_END {
                                if self.temp[self.idx + len as usize - 4] == TOKEN_MID {
                                    self.len = Some(len);
                                    self.st = SpState::ExpectEnd;
                                    self.bp = Some(self.idx);
                                    self.idx = self.idx + len as usize + 1;
                                    continue 'Out;
                                } else {
                                    println!("{:?} i={} len={} bad package not found TOKEN_MID!",self.temp,self.idx,len);
                                    self.temp.pop();
                                    return None;
                                }
                            } else {
                                println!("{:?} i={} len={} bad package not found TOKEN_END!",self.temp,self.idx,len);
                                self.temp.pop();
                                return None;
                            }
                        } else {
                            self.temp.pop();
                        }
                    }
                    return None;
                }
                SpState::ExpectLen => {}
                SpState::Content => {}
                SpState::ExpectMid => {}
                SpState::ExpectExt => {}
                SpState::ExpectEnd => {
                    let b = self.bp.unwrap();
                    let res = self.temp[(b+1)..self.idx].to_vec();
                    if self.temp.len() - self.idx > 1
                    {
                        let mut rest_data = Vec::with_capacity(self.temp.len() - (self.idx+1));
                        rest_data.resize(self.temp.len() - (self.idx+1),0);
                        rest_data.copy_from_slice(&(self.temp[(self.idx+1)..]));
                        self.temp = rest_data;
                        self.need_ck = true;
                    }else{
                        self.temp.clear();
                    }
                    self.st = SpState::ExpectBegin;
                    self.len = None;
                    self.bp = None;
                    self.idx = 0;
                    self.ext = None;
                    return Some(res);
                }
            }
        }
        None
    }

    fn need_check(&self) -> bool {
        !self.temp.is_empty() && self.need_ck
    }
}