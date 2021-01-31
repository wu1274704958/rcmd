use crate::agreement::DataTransform;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::{Write, Read};
use flate2::read::{GzDecoder, ZlibDecoder};

pub struct DefCompress{

}

impl DataTransform for DefCompress {
    fn to(&self, d: &[u8]) -> Vec<u8> {
        let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
        e.write_all(d);
        if let Ok(v) = e.finish(){
            //dbg!(&v);
            v
        }else{
            vec![]
        }
    }

    fn form(&self, d: &[u8]) -> Vec<u8> {
        let mut d = ZlibDecoder::new(d);
        let mut res = vec![];
        let r = d.read_to_end(&mut res);
        //dbg!(r);
        res
    }
}