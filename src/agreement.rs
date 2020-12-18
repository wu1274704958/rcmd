use crate::tools::{u32_form_bytes, TOKEN_BEGIN, TOKEN_END};
use std::mem::size_of;
use async_std::sync::Arc;
use std::sync::Mutex;

pub trait DataTransform<In,Out> : Send + Sync{
    fn to(&self,d:&In)->Out;
    fn form(&self,d:&Out)->In;
}

pub trait Agreement<'a> {
    type AgreementTy;
    fn parse(&self,data:&'a Vec<u8>)->Option<Self::AgreementTy>;
    fn package(&self,data:Vec<u8>,ext:u32)->Vec<u8>;
    fn add_transform(&mut self,dt:Arc<dyn DataTransform<Vec<u8>,Vec<u8>>>);
    fn get_transform(&self,id:usize)->&dyn DataTransform<Vec<u8>,Vec<u8>>;
    fn transform_count(&self)->usize;
    fn parse_tf(&self,data:&'a mut Vec<u8>)->Option<Self::AgreementTy>
    {
        if self.transform_count() > 0
        {
            let mut i:usize = self.transform_count() - 1;
            let mut tf_data = data.clone();
            let mut res_data;
            loop{
                let tf = self.get_transform(i);
                res_data = tf.form(&tf_data);
                tf_data = res_data;
                if i == 0 {break;}
                i -= 1;
            }
            *data = tf_data;
            self.parse(data)
        }else {
            self.parse(data)
        }
    }

    fn package_tf(&self,data:Vec<u8>,ext:u32)->Vec<u8>
    {
        let mut res = self.package(data,ext);
        let mut res_tf;
        for i in 0..self.transform_count()
        {
            let tf = self.get_transform(i);
            res_tf = tf.to(&res);
            res = res_tf;
        }
        res
    }
}


pub struct DefParser {
    tfs:Vec<Arc<dyn DataTransform<Vec<u8>,Vec<u8>>>>
}

impl DefParser {
    pub fn new() -> DefParser
    {
        DefParser{
            tfs:Vec::new()
        }
    }
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

#[derive(Copy, Clone)]
pub struct TestDataTransform{

}

#[derive(Copy, Clone)]
pub struct Test2DataTransform{

}

impl DataTransform<Vec<u8>,Vec<u8>> for TestDataTransform
{
    fn to(&self,d: &Vec<u8>) -> Vec<u8> {
        let mut res = d.clone();
        if res[4] < u8::max_value() {
            res[4] += 1;
        }else{
            res[4] = u8::min_value();
        }
        res
    }

    fn form(&self,d: &Vec<u8>) -> Vec<u8> {
        let mut res = d.clone();
        if res[4] > u8::min_value() {
            res[4] -= 1;
        }else{
            res[4] = u8::max_value();
        }
        res
    }
}

impl DataTransform<Vec<u8>,Vec<u8>> for Test2DataTransform
{
    fn to(&self,d: &Vec<u8>) -> Vec<u8> {
        let mut res = d.clone();
        res[4] /= 2;
        res
    }

    fn form(&self,d: &Vec<u8>) -> Vec<u8> {
        let mut res = d.clone();
        res[4] *= 2;
        res
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

        for i in len_buf.iter(){
            //dbg!(i);
            res.push(*i);
        }
        res.append(&mut data);
        for i in ext_buf.iter(){
            res.push(*i);
        }

        res
    }

    fn add_transform(&mut self, dt: Arc<dyn DataTransform<Vec<u8>,Vec<u8>>>) {
        self.tfs.push(dt);
    }

    fn get_transform(&self, id: usize) -> &dyn DataTransform<Vec<u8>, Vec<u8>> {
        self.tfs[id].as_ref()
    }

    fn transform_count(&self) -> usize {
        self.tfs.len()
    }
}