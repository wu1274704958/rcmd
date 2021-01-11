use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use crate::client_handlers::def_handler::SubHandle;
use crate::tools::ext_content;
use crate::ext_code::*;
use std::io::Write;
use crate::tools::{TOKEN_BEGIN,TOKEN_END};
use std::mem::size_of;
use getopts::HasArg::No;

pub struct SaveFile
{
    file_map:Arc<Mutex<HashMap<String,File>>>
}

impl SaveFile {
    pub fn new()->SaveFile
    {
        SaveFile{
            file_map:Arc::new(Mutex::new(HashMap::new()))
        }
    }
}

fn ret_ext(id_buf:&[u8],ext:u32)->Vec<u8>
{
    let mut v = Vec::with_capacity(id_buf.len());
    v.resize(id_buf.len(),0);
    v.copy_from_slice(id_buf);
    v.append(&mut ext_content(ext));
    v
}

fn ret(id_buf:&[u8],mut v2:Vec<u8>)->Vec<u8>
{
    let mut v = Vec::with_capacity(id_buf.len());
    v.resize(id_buf.len(),0);
    v.copy_from_slice(id_buf);
    v.append(&mut v2);
    v
}

impl SubHandle for SaveFile
{
    fn handle(&self, data: &[u8], len: u32, ext: u32) -> Option<(Vec<u8>, u32)> {
        if ext != EXT_SAVE_FILE_CREATE &&
            ext != EXT_SAVE_FILE &&
            ext != EXT_SAVE_FILE_ELF{
            return None;
        }
        let mut id_buf = [0u8;size_of::<usize>()];
        id_buf.copy_from_slice(&data[0..size_of::<usize>()]);

        let data = &data[size_of::<usize>()..];

        if data[0] != TOKEN_BEGIN
        {
            return Some((ret_ext(&id_buf,EXT_AGREEMENT_ERR_CODE),EXT_ERR_SAVE_FILE_RET_EXT));
        }

        let mut name_e = 0usize;
        loop  {
            if name_e >= data.len()
            {
                return Some((ret_ext(&id_buf,EXT_AGREEMENT_ERR_CODE),EXT_ERR_SAVE_FILE_RET_EXT));
            }else if *data.get(name_e.clone()).unwrap() == TOKEN_END
            {
                break;
            }
            name_e = name_e + 1;
        }
        let name = String::from_utf8_lossy(&data[1..name_e]).trim().to_string();

        if name.is_empty()
        {
            return Some((ret_ext(&id_buf,EXT_ERR_FILE_NAME_EMPTY),EXT_ERR_SAVE_FILE_RET_EXT));
        }

        let mut rd = name.as_bytes().to_vec();

        if ext == EXT_SAVE_FILE_CREATE{

            if let Ok(m) = self.file_map.lock(){
                if let Some(v) = m.get(&name)
                {
                    let mut ve = ret_ext(&id_buf,EXT_ERR_ALREADY_CREATED);
                    ve.append(&mut rd);
                    return Some((ve, EXT_ERR_SAVE_FILE_RET_EXT));
                }
            }

            if let Ok(mut f) = OpenOptions::new().create(true).append(false).write(true).open(name.clone())
            {
                if let Ok(l) = f.write(&data[(name_e+1)..])
                {
                    let b = (l as u32).to_be_bytes();
                    b.iter().for_each(|it|{rd.push(*it)});
                    if let Ok(mut fm) = self.file_map.lock()
                    {
                        fm.insert(name.clone(), f);
                        return Some((ret(&id_buf,rd), EXT_SAVE_FILE_CREATE));
                    }
                }else{
                    let mut ve = ret_ext(&id_buf,EXT_ERR_WRITE_FILE_FAILED);
                    ve.append(&mut rd);
                    return Some((ve, EXT_ERR_SAVE_FILE_RET_EXT));
                }
            }else{
                let mut ve = ret_ext(&id_buf,EXT_ERR_CREATE_FILE_FAILED);
                ve.append(&mut rd);
                return Some((ve, EXT_ERR_SAVE_FILE_RET_EXT));
            }
        } else if ext == EXT_SAVE_FILE{

            if let Ok(mut fm) = self.file_map.lock()
            {
                if let Some(f) = fm.get_mut(&name)
                {
                    let file_data = &data[(name_e.clone()+1)..];
                    if let Ok(l) = f.write(file_data)
                    {
                        let b = (l as u32).to_be_bytes();
                        b.iter().for_each(|it|{rd.push(*it)});
                        //f.1.sync_all().unwrap();
                        return Some((ret(&id_buf,rd),EXT_SAVE_FILE));
                    }else{
                        let mut ve = ret_ext(&id_buf,EXT_ERR_WRITE_FILE_FAILED);
                        ve.append(&mut rd);
                        return Some((ve, EXT_ERR_SAVE_FILE_RET_EXT));
                    }
                }else {
                    let mut ve = ret_ext(&id_buf,EXT_ERR_FILE_NAME_NOT_EXITS);
                    ve.append(&mut rd);
                    return Some((ve, EXT_ERR_SAVE_FILE_RET_EXT));
                }
            }

        }else if ext == EXT_SAVE_FILE_ELF{
            if let Ok(mut fm) = self.file_map.lock()
            {
                if let Some(f) = fm.get_mut(&name)
                {
                    f.sync_all().unwrap();
                }else{
                    let mut ve = ret_ext(&id_buf,EXT_ERR_FILE_NAME_NOT_EXITS);
                    ve.append(&mut rd);
                    return Some((ve, EXT_ERR_SAVE_FILE_RET_EXT));
                }
                if let Some(k) = fm.remove(&name)
                {
                    return Some((ret(&id_buf,rd),EXT_SAVE_FILE_ELF));
                }
            }
        }
        None
    }
}