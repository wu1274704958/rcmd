use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::{Write, stdin};
use std::fs::OpenOptions;

fn to(d: &Vec<u8>) -> Vec<u8> {
    let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
    e.write_all(d.as_slice());
    if let Ok(v) = e.finish(){
        //dbg!(&v);
        v
    }else{
        vec![]
    }
}

pub fn compress(s:&str)->Vec<u8>
{
    let v = s.as_bytes().to_vec();
    to(&v)
}

fn main()
{
    let mut cnf = OpenOptions::new().write(true).append(false).create(true).open("cnf").unwrap();
    let mut line = String::new();
    stdin().read_line(&mut line);

    let v = compress(line.as_str().trim());
    cnf.write_all(v.as_ref());
    cnf.sync_all();
}