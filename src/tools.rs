use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use crate::ab_client::{AbClient, State};

pub fn del_client(cs:& mut Arc<Mutex<HashMap<usize,Box<AbClient>>>>, id:usize)
{
    let mut cs_ = cs.lock().unwrap();
    if cs_.contains_key(&id)
    {
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

pub fn get_client_write_buf(cs:& mut Arc<Mutex<HashMap<usize,Box<AbClient>>>>,id:usize)->Option<Vec<u8>>
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

pub fn read_form_buf(reading:&mut bool,buf:&[u8],n:usize,data:&mut Vec<u8>,buf_rest:&mut [u8],buf_rest_len:&mut usize)->bool{
    let mut has_rest = false;
    let mut end_idx = 0usize;
    for i in 0..n{
        if !(*reading){
            if buf[i] == TOKEN_BEGIN{
                *reading = true;
                continue;
            }
        }else{
            if buf[i] == TOKEN_END{
                *reading = false;
                has_rest = true;
                end_idx = i;
                break;
            }else{
                data.push(buf[i]);
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

pub fn handle_request<'a>(reading:&mut bool,data:&mut Vec<u8>,buf_rest:&mut [u8],buf_rest_len:usize,f:&'a dyn Fn(&mut Vec<u8>))
{
    if !(*reading) && !data.is_empty(){
        // handle
        f(data);
        data.clear();
        if buf_rest_len > 0{
            let mut rest = [0u8;1024];
            let mut rest_len = 0usize;
            read_form_buf(reading,&buf_rest,buf_rest_len,data,&mut rest,&mut rest_len);
            handle_request(reading,data,&mut rest,rest_len,f);
        }
    }
}


pub const TOKEN_BEGIN:u8 = 7u8;
pub const TOKEN_END:u8 = 9u8;