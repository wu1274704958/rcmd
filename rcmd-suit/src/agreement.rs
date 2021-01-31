use crate::tools::{u32_form_bytes, set_slices_form_u32, TOKEN_BEGIN, TOKEN_END, TOKEN_MID, TOKEN_NORMAL};
use std::mem::size_of;
use async_std::sync::Arc;
use std::sync::Mutex;

pub trait DataTransform : Send + Sync{
    fn to(&self,d:&[u8])->Vec<u8>;
    fn form(&self,d:&[u8])->Vec<u8>;
}

pub trait Agreement {
    fn parse<'a>(&self,data:&'a Vec<u8>)->Option<Message<'a>>;
    fn package(&self,data:Vec<u8>,ext:u32,pkg_tag:u8)->Vec<u8>;
    fn add_transform(&mut self,dt:Arc<dyn DataTransform>);
    fn get_transform(&self,id:usize)->&dyn DataTransform;
    fn transform_count(&self)->usize;
    fn parse_tf<'a>(&self,data:&'a mut Vec<u8>)->Option<Message<'a>>
    {
        if self.transform_count() > 0
        {
            let mut i:usize = self.transform_count() - 1;
            let mut l:[u8;4] = [0,0,0,0];
            let mut e:[u8;4] = [0,0,0,0];
            let mut tag = TOKEN_NORMAL;
            e.copy_from_slice(&data[(data.len() - size_of::<u32>())..data.len()]);
            tag = data[size_of::<u32>()];
            //dbg!(e);
            let mut tf_data = &data[(size_of::<u32>() + size_of::<u8>())..(data.len() - 5)];
            //println!("decompress before = {:?} len = {} ",&tf_data,tf_data.len());
            let mut res_data;
            loop{
                let tf = self.get_transform(i);
                res_data = tf.form(tf_data);
                tf_data = &res_data[..];
                if i == 0 {break;}
                i -= 1;
            }
            *data = tf_data.to_vec();
            //println!("decompress after = {:?} len = {} ",&data,data.len());
            set_slices_form_u32(&mut l,(data.len() + 10) as u32);
            for i in l.iter().enumerate() {
                data.insert(i.0,*i.1);
            }
            data.insert(size_of::<u32>(),tag);
            data.push(TOKEN_MID);
            for i in e.iter() {
                data.push(*i);
            }
            self.parse(data)
        }else {
            self.parse(data)
        }
    }

    fn package_tf(&self,mut data:Vec<u8>,ext:u32,pkg_tag:u8)->Vec<u8>
    {
        //println!("compress before = {:?} len = {}",&data,data.len());
        let mut res_tf;
        for i in 0..self.transform_count()
        {
            let tf = self.get_transform(i);
            res_tf = tf.to(&data[..]);
            data = res_tf;
        }
        //println!("compress after = {:?} len = {}",&data,data.len());
        self.package(data,ext,pkg_tag)
    }

    fn package_nor(&self,mut data:Vec<u8>,ext:u32)->Vec<u8>
    {
        self.package_tf(data,ext,TOKEN_NORMAL)
    }
}


pub struct DefParser {
    tfs:Vec<Arc<dyn DataTransform>>
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
    pub ext:u32,
    pub tag:u8
}

impl <'a> Message<'a>{
    fn new(len:u32,msg:&'a [u8],ext:u32,tag:u8)->Message<'a>
    {
        Message{
            len,msg,ext,tag
        }
    }
}

#[derive(Copy, Clone)]
pub struct TestDataTransform{

}

#[derive(Copy, Clone)]
pub struct Test2DataTransform{

}

impl DataTransform for TestDataTransform
{
    fn to(&self,d: &[u8]) -> Vec<u8> {
        let mut res = d.to_vec();
        if res[4] < u8::max_value() {
            res[4] += 1;
        }else{
            res[4] = u8::min_value();
        }
        res
    }

    fn form(&self,d: &[u8]) -> Vec<u8> {
        let mut res = d.to_vec();
        if res[4] > u8::min_value() {
            res[4] -= 1;
        }else{
            res[4] = u8::max_value();
        }
        res
    }
}

impl DataTransform for Test2DataTransform
{
    fn to(&self,d: &[u8]) -> Vec<u8> {
        let mut res = d.to_vec();
        res[4] /= 2;
        res
    }

    fn form(&self,d: &[u8]) -> Vec<u8> {
        let mut res = d.to_vec();
        res[4] *= 2;
        res
    }
}

impl Agreement for DefParser
{
    fn parse<'a>(&self,data:&'a Vec<u8>)->Option<Message<'a>>{
        let len = u32_form_bytes(data.as_slice());
        //dbg!(len);
        if len as usize != data.len()
        {
            return None
        }

        let h_m = data.split_at(size_of::<u32>() + size_of::<u8>());
        let hml = h_m.1.len();
        let m_d = h_m.1.split_at(hml - size_of::<u32>());
        //dbg!(&m_d);
        let ext = u32_form_bytes(m_d.1);
        let pkg_tag = h_m.0[size_of::<u32>()];
        //dbg!(ext);
        Some(Message::new(len,&m_d.0[0..(m_d.0.len() - 1)],ext,pkg_tag))
    }

    fn package(&self, mut data:Vec<u8>,ext:u32,pkg_tag:u8) -> Vec<u8> {
        let mut res = Vec::new();
        let len = data.len() as u32 + (size_of::<u32>() as u32 * 2) + (size_of::<u8>() * 2) as u32;
        //dbg!(len);
        let len_buf = len.to_be_bytes();
        let ext_buf = ext.to_be_bytes();

        res.push(TOKEN_BEGIN);

        for i in len_buf.iter(){
            //dbg!(i);
            res.push(*i);
        }
        res.push(pkg_tag);
        res.append(&mut data);
        res.push(TOKEN_MID);
        for i in ext_buf.iter(){
            res.push(*i);
        }
        res.push(TOKEN_END);
        res
    }

    fn add_transform(&mut self, dt: Arc<dyn DataTransform>) {
        self.tfs.push(dt);
    }

    fn get_transform(&self, id: usize) -> &dyn DataTransform {
        self.tfs[id].as_ref()
    }

    fn transform_count(&self) -> usize {
        self.tfs.len()
    }
}
