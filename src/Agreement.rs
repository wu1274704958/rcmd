use crate::tools::{u32_form_bytes, TOKEN_BEGIN, TOKEN_END};
use std::mem::size_of;

pub trait Agreement<'a>{
    type AgreementTy;
    fn parse(&self,data:&'a Vec<u8>)->Option<Self::AgreementTy>;
    fn package(&self,data:Vec<u8>,ext:u32)->Vec<u8>;
}

#[derive(Copy, Clone)]
pub struct DefParser{

}

#[derive(Copy, Clone,Debug)]
pub struct Message<'a>{
    pub len:u32,
    pub msg:&'a [u8],
    pub ext:u32
}

impl <'a> Message<'a>{
    fn new(len:u32,msg:&'a [u8],ext:u32)->Message<'a>
    {
        Message{
            len,msg,ext
        }
    }
}

impl <'a>Agreement<'a> for DefParser
{
    type AgreementTy = Message<'a>;

    fn parse(&self,data: &'a Vec<u8>) -> Option<Self::AgreementTy> {
        let len = u32_form_bytes(data.as_slice());
        //dbg!(len);
        if len as usize != data.len()
        {
            return None
        }

        let h_m = data.split_at(4);
        let hml = h_m.1.len();
        let m_d = h_m.1.split_at(hml - 4);
        //dbg!(&m_d);
        let ext = u32_form_bytes(m_d.1);
        //dbg!(ext);
        Some(Message::new(len,m_d.0,ext))
    }

    fn package(&self, mut data:Vec<u8>,ext:u32) -> Vec<u8> {
        let mut res = Vec::new();
        let len = data.len() as u32 + size_of::<u32>() as u32 * 2;
        //dbg!(len);
        let len_buf = len.to_be_bytes();
        let ext_buf = ext.to_be_bytes();
        res.push(TOKEN_BEGIN);
        for i in len_buf.iter(){
            //dbg!(i);
            res.push(*i);
        }
        res.append(&mut data);
        for i in ext_buf.iter(){
            res.push(*i);
        }
        res.push(TOKEN_END);
        res
    }
}
