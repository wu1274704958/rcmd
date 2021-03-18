use std::sync::{Mutex, Arc};
use crate::extc::*;
use crate::model;
use terminal::Color;
use std::collections::VecDeque;
use std::mem::size_of;
use std::path::PathBuf;
use std::str::FromStr;
use std::convert::Infallible;
use std::fs::{OpenOptions, File};
use std::io::Read;
use std::ffi::OsStr;
use rcmd_suit::client_handler::SubHandle;
use rcmd_suit::tools::{TOKEN_BEGIN, TOKEN_END, SEND_BUF_SIZE, ext_content};

pub struct PullFile
{
    queue:Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>
}

impl PullFile {
    pub fn new(queue:Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>)->PullFile
    {
        PullFile{
            queue
        }
    }
}

impl SubHandle for PullFile
{
    fn handle(&self, data: &[u8], len: u32, ext: u32) -> Option<(Vec<u8>, u32)> {
        
        let mut id_buf = data[0..size_of::<usize>()].to_vec();
        let name = String::from_utf8_lossy(&data[size_of::<usize>()..]).to_string();
        if let Ok(msg) = serde_json::from_str::<model::PullFileMsg>(name.as_str())
        {
            let path = PathBuf::from_str(msg.far_end_path.as_ref());
            match path {
                Ok(p) => {
                    let filename = if let Some(s) = msg.near_end_path{
                        s
                    }else{
                        match p.file_name() {
                            None => {
                                id_buf.append(&mut ext_content(EXT_ERR_BAD_FILE_PATH));
                                return Some((id_buf, EXT_ERR_PULL_FILE_RET_EXT));
                            }
                            Some(p) => {
                                p.to_str().unwrap().trim().to_string()
                            }
                        }
                    };
                    match OpenOptions::new().read(true).open(p)
                    {
                        Ok(mut f) => {
                            id_buf.push(TOKEN_BEGIN);
                            id_buf.extend_from_slice(filename.as_bytes());
                            id_buf.push(TOKEN_END);

                            let mut buf = Vec::with_capacity(SEND_BUF_SIZE);
                            buf.resize(SEND_BUF_SIZE, 0);
                            let mut is_first = true;
                            loop {
                                let mut d = id_buf.clone();
                                match f.read(&mut buf[..]) {
                                    Ok(n) => {
                                        if n <= 0
                                        {
                                            send(&self.queue, d, EXT_SEND_FILE_ELF);
                                            break;
                                        } else {
                                            d.reserve(n);
                                            d.extend_from_slice(&buf[0..n]);
                                            send(&self.queue, d, if is_first { EXT_SEND_FILE_CREATE } else { EXT_SEND_FILE });
                                            is_first = false;
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            return Some((id_buf, EXT_PULL_FILE_C));
                        }
                        Err(_) => {
                            id_buf.append(&mut ext_content(EXT_ERR_OPEN_FILE));
                            return Some((id_buf, EXT_ERR_PULL_FILE_RET_EXT));
                        }
                    }
                }
                Err(_) => {
                    id_buf.append(&mut ext_content(EXT_ERR_BAD_FILE_PATH));
                    return Some((id_buf, EXT_ERR_PULL_FILE_RET_EXT));
                }
            };
        }else{
            id_buf.append(&mut ext_content(EXT_ERR_PARSE_ARGS));
            return Some((id_buf, EXT_ERR_PULL_FILE_RET_EXT));
        }
        
        None
    }
    fn interested(&self, ext:u32) ->bool {
        ext == EXT_PULL_FILE_C
    }
}

fn send(queue: &Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>, data: Vec<u8>,ext:u32) {
    let mut a = queue.lock().unwrap();
    {
        a.push_back((data,ext));
    }
}