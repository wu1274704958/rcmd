use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use crate::ab_client::{AbClient, State};
use tokio::net::TcpListener;
use crate::handler::{Handle, SubHandle, DefHandler};
use crate::agreement::{Agreement, Message};
use crate::config_build::Config;
use std::error::Error;
use std::env;
use std::net::{Ipv4Addr, SocketAddr};
use std::str::FromStr;
use async_std::net::SocketAddrV4;


pub const TOKEN_BEGIN:u8 = 7u8;
pub const TOKEN_MID:u8 = 31u8;
pub const TOKEN_END:u8 = 9u8;

#[cfg(target_endian = "little")]
pub const BigEndian:bool = false;

#[cfg(target_endian = "big")]
pub const BigEndian:bool = true;

pub fn del_client(cs:& mut Arc<Mutex<HashMap<usize,Box<AbClient>>>>, id:usize)
{
    let mut cs_ = cs.lock().unwrap();
    if cs_.contains_key(&id)
    {
        println!("del client {} -=-=-=-=-",id);
        cs_.remove(&id).unwrap();
    }
}

pub fn set_client_st(cs:& mut Arc<Mutex<HashMap<usize,Box<AbClient>>>>,id:usize,st:State)
{
    let mut cs_ = cs.lock().unwrap();
    if let Some(c) = cs_.get_mut(&id)
    {
        c.state = st;
    }
}

pub fn get_client_write_buf(cs:& mut Arc<Mutex<HashMap<usize,Box<AbClient>>>>,id:usize)->Option<(Vec<u8>,u32)>
{
    let mut cs_ = cs.lock().unwrap();
    if let Some(c) = cs_.get_mut(&id)
    {
        let res = c.write_buf.clone();
        c.write_buf = None;
        return res;
    }
    None
}

pub fn get_client_st(cs:&Arc<Mutex<HashMap<usize,Box<AbClient>>>>,id:usize)->Option<State>
{
    let mut cs_ = cs.lock().unwrap();
    if let Some(c) = cs_.get(&id)
    {
        return Some(c.state);
    }
    None
}

pub fn read_form_buf(reading:&mut bool,buf:&[u8],n:usize,data:&mut Vec<u8>,buf_rest:&mut [u8],buf_rest_len:&mut usize)->bool{
    let mut has_rest = false;
    let mut end_idx = 0usize;
    let mut len = 0u32;
    for i in 0..n{
        if !(*reading){
            if buf[i] == TOKEN_BEGIN{
                len = u32_form_bytes(&buf[i+1..]);
                if len == 0 {continue;}
                *reading = true;
                continue;
            }
        }else{
            if data.len() < len as usize
            {
                data.push(buf[i]);
            }else{
                if buf[i] == TOKEN_END {
                     *reading = false;
                     has_rest = true;
                     end_idx = i;
                     break;
                }else {
                    println!("lost the magic number!!!");
                    *reading = false;
                    data.clear();
                    continue;
                }
            }
        }
    }
    if has_rest && end_idx < n
    {
        let mut j = 0;
        for i in end_idx..n {
            buf_rest[j] = buf[i];
            j += 1;
        }
        *buf_rest_len = j;
    }

    has_rest && end_idx < n
}

pub fn handle_request(reading:&mut bool,data:&mut Vec<u8>,buf_rest:&mut [u8],buf_rest_len:usize,result:&mut Vec<Vec<u8>>)
{
    if !(*reading) && !data.is_empty(){
        // handle
        result.push(data.clone());
        data.clear();
        if buf_rest_len > 0{
            let mut rest = [0u8;1024];
            let mut rest_len = 0usize;
            read_form_buf(reading,&buf_rest,buf_rest_len,data,&mut rest,&mut rest_len);
            handle_request(reading,data,&mut rest,rest_len,result);
        }
    }
}

pub fn handle_request_ex<'a>(reading:&mut bool,data:&mut Vec<u8>,buf_rest:&mut [u8],buf_rest_len:usize,f:&'a mut dyn FnMut(&mut Vec<u8>))
{
    if !(*reading) && !data.is_empty(){
        // handle
       f(data);
        data.clear();
        if buf_rest_len > 0{
            let mut rest = [0u8;1024];
            let mut rest_len = 0usize;
            read_form_buf(reading,&buf_rest,buf_rest_len,data,&mut rest,&mut rest_len);
            handle_request_ex(reading,data,&mut rest,rest_len,f);
        }
    }
}

pub fn u32_form_bytes(b:&[u8])->u32
{
    if b.len() < 4{ return 0; }
    let mut a = [0u8;4];
    a.copy_from_slice(&b[0..4]);
    u32::from_be_bytes(a)
}

pub fn set_slices_form_u32(b:&mut [u8],v:u32)
{
    if b.len() < 4{ return; }
    let a = v.to_be_bytes();
    for i in 0..4{
        b[i] = a[i];
    }
}

pub fn real_package(mut pkg:Vec<u8>)->Vec<u8>
{
    let mut real_pkg = Vec::new();
    real_pkg.push(TOKEN_BEGIN);
    real_pkg.append(&mut pkg);
    real_pkg.push(TOKEN_END);
    real_pkg
}


pub fn parse_args(mut p0: Config) ->Config {
    let args = env::args();
    let mut ip = Ipv4Addr::new(127, 0, 0, 1);
    let mut port = 8080u16;
    if args.len() > 1
    {
        args.enumerate().for_each(|it|
            {
                if it.0 == 1
                {
                    if let Ok(i) = Ipv4Addr::from_str(it.1.as_str())
                    {
                        ip = i;
                    }
                }
                if it.0 == 2
                {
                    if let Ok(p) = u16::from_str(it.1.as_str())
                    {
                        port = p;
                    }
                }
            });
        p0.addr = SocketAddr::V4(SocketAddrV4::new(ip,port));
    }
    p0
}



