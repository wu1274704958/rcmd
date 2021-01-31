use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use crate::ab_client::{AbClient, State};
use tokio::net::{TcpListener, UdpSocket};
use crate::handler::{Handle, SubHandle, DefHandler};
use crate::agreement::{Agreement, Message, DataTransform};
use crate::config_build::Config;
use std::error::Error;
use std::env;
use std::net::{Ipv4Addr, SocketAddr};
use std::str::FromStr;
use async_std::net::SocketAddrV4;
use args::{ArgsError, Args};
use getopts::Occur;
use std::ffi::OsStr;
use std::mem::size_of;


pub const TOKEN_BEGIN:u8 = 7u8;
pub const TOKEN_MID:u8 = 31u8;
pub const TOKEN_END:u8 = 9u8;

pub const TOKEN_NORMAL:u8 = 1u8;
pub const TOKEN_SUBPACKAGE_BEGIN:u8 = 2u8;
pub const TOKEN_SUBPACKAGE:u8 = 3u8;
pub const TOKEN_SUBPACKAGE_END:u8 = 4u8;

#[cfg(target_endian = "little")]
pub const BigEndian:bool = false;

#[cfg(target_endian = "big")]
pub const BigEndian:bool = true;


pub const SEND_BUF_SIZE:usize = 1024 * 100;

pub fn del_client(cs:& mut Arc<Mutex<HashMap<usize,Box<AbClient>>>>, id:usize) ->usize
{
    let mut cs_ = cs.lock().unwrap();
    if cs_.contains_key(&id)
    {
        println!("del client {} -=-=-=-=-",id);
        cs_.remove(&id).unwrap();
    }
    return cs_.len();
}

pub fn set_client_st(cs:&Arc<Mutex<HashMap<usize,Box<AbClient>>>>,id:usize,st:State)
{
    let mut cs_ = cs.lock().unwrap();
    if let Some(c) = cs_.get_mut(&id)
    {
        c.state = st;
    }
}

pub fn get_client_write_buf(cs:& mut Arc<Mutex<HashMap<usize,Box<AbClient>>>>,id:usize)->Option<(Vec<u8>,u32)>
{
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

pub fn ext_content(content:u32)->Vec<u8>
{
    let mut v = Vec::with_capacity(size_of::<u32>());
    v.resize(size_of::<u32>(),0);
    set_slices_form_u32(&mut v[..],content);
    v
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

#[derive(Debug)]
pub struct ClientArgs{
    pub ip : Ipv4Addr,
    pub port : u16,
    pub bind_port : u16,
    pub acc : Option<String>,
    pub pwd : Option<String>,
    pub thread_num:u16
}

pub fn parse_c_args() -> Result<ClientArgs, ArgsError> {
    let input:Vec<_> = std::env::args().collect();
    parse_c_args_ex(input)
}
pub fn parse_c_args_ex<C: IntoIterator>(input:C) -> Result<ClientArgs, ArgsError>
    where C::Item: AsRef<OsStr>{
    let mut args = Args::new("Client", "-=-=-=-=-=-=-=-=-=-=-");
    args.flag("h", "help", "Print the usage menu");
    args.option("i",
                "ip",
                "IP of will connect server",
                "IP",
                Occur::Optional,
                Some("127.0.0.1".to_string()));
    args.option("p",
                "port",
                "Port of will connect server",
                "PORT",
                Occur::Optional,
                Some(String::from("8080")));
    args.option("b",
                "bind port",
                "Port of will be bind",
                "BIND PORT",
                Occur::Optional,
                Some(String::from("8081")));
    args.option("a",
                "Account",
                "Account for auto login",
                "ACC",
                Occur::Optional,
                None);
    args.option("s",
                "password",
                "Password for auto login",
                "PWD",
                Occur::Optional,
                None);
    args.option("j",
                "thread",
                "Thread num",
                "Thread Num",
                Occur::Optional,
                Some("1".to_string()));

    args.parse(input)?;

    let help = args.value_of("help")?;
    if help {
        println!("{}",args.full_usage());
        return Err(ArgsError::new("","show help"));
    }
    let mut ip = Ipv4Addr::new(127,0,0,1);
    let mut port = 8080u16;
    let mut bind_port = 8081u16;
    let mut acc = None;
    let mut pwd = None;
    let mut thread_num = 1u16;
    args.iter().for_each(|(k,v)|{
        match k.as_str() {
            "ip" => {
                match Ipv4Addr::from_str(v.as_str())
                {
                    Ok(i) => { ip = i;}
                    Err(e) => { }
                }
            }
            "port" => {
                match u16::from_str(v.as_str())
                {
                    Ok(i) => { port = i;}
                    Err(e) => { }
                }
            }
            "bind port" => {
                match u16::from_str(v.as_str())
                {
                    Ok(i) => { bind_port = i;}
                    Err(e) => { }
                }
            }
            "Account" => {
                acc = Some(v.clone());
            }
            "password" => {
                pwd = Some(v.clone());
            }
            "thread" => {
                if let Ok(tn) = u16::from_str(v.as_str()){
                    thread_num = tn;
                };
            }
            _=>{}
        }
    });

    if acc.is_none() || pwd.is_none() {  acc = None; pwd = None;  }
    Ok(ClientArgs{
        ip,port,acc,pwd,thread_num,bind_port
    })
}

pub fn compress(s:&str)->Vec<u8>
{
    let c = crate::data_transform::def_compress::DefCompress{};
    let v = s.as_bytes().to_vec();
    c.to(&v)
}

pub fn decompress(s:&Vec<u8>)->String
{
    let c = crate::data_transform::def_compress::DefCompress{};
    let res = c.form(&s);
    String::from_utf8_lossy(res.as_ref()).to_string()
}

#[cfg(target_os = "windows")]
use winapi::um::winsock2::{SOCKET,WSAIoctl,LPWSAOVERLAPPED,WSAOVERLAPPED};
#[cfg(target_os = "windows")]
use winapi::ctypes::{c_void,c_ulong};
#[cfg(target_os = "windows")]
use std::ptr::null;
#[cfg(target_os = "windows")]
use std::os::windows::prelude::*;
#[cfg(target_os = "windows")]
pub fn platform_handle(s:&UdpSocket)
{
    let s = s.as_raw_socket();

    let mut bEnalbeConnRestError = 0u32;
    let ptr = &mut bEnalbeConnRestError as *mut u32 as *mut c_void;
    let mut dwBytesReturned:c_ulong = 0;
    let p2 = &mut dwBytesReturned as *mut c_ulong;

    unsafe { WSAIoctl(
        s as usize,
        2550136844,
        ptr,
        4,
        null::<c_void>() as *mut c_void,
        0,
        p2,
        null::<WSAOVERLAPPED>() as *mut WSAOVERLAPPED,
        None); }
}

#[cfg(not(target_os = "windows"))]
pub fn platform_handle(s:&UdpSocket)
{

}

