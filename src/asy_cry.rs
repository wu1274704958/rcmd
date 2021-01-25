
// ext 9        heartbeat
// ext 0        default
// ext 10 .. 20  asymmetric cryptographic
// ext 20 .. 40  asymmetric cryptographic err
// ext 40 41      upload file

use rsa::{PublicKey, RSAPrivateKey, PaddingScheme, RSAPublicKey, PublicKeyParts, BigUint};
use rsa::errors::Error;
use crate::tools::*;
use std::collections::HashSet;
use crate::ext_code::*;
use async_trait::async_trait;
use tokio::runtime;
use std::cell::Cell;

#[derive(Debug)]
pub enum EncryptRes {
    EncryptSucc(Vec<u8>),
    RPubKey((Vec<u8>, u32)),
    ErrMsg((Vec<u8>, u32)),
    NotChange,
    Break,
}
#[async_trait]
pub trait AsyCry{

    async fn build_pub_key(&mut self)->Result<Vec<u8>,u32>
    {
        Err(20) // 20 not impl
    }

    async fn real_build_pub_key(bit_size:usize) -> Result<(Vec<u8>,Option<RSAPrivateKey>),u32>;


    fn pub_key_from(&self,d:&[u8],ext:u32) ->Result<RSAPublicKey,u32>
    {
        Err(20)
    }

    async fn try_decrypt(&mut self, d:&[u8],ext:u32) -> EncryptRes {
        EncryptRes::NotChange
    }

    fn encrypt(&self,d:&[u8],ext:u32) -> EncryptRes {
        EncryptRes::NotChange
    }

    fn can_encrypt(&self)->bool
    {
        true
    }
}

pub struct DefAsyCry{
    pri_key: Option<RSAPrivateKey>,
    pub_key: Option<RSAPublicKey>,
    ignore_map: HashSet<u32>,
    oth_ready: bool,
    state:u32,
    rt:Cell<Option<runtime::Runtime>>
}

impl DefAsyCry
{
    pub fn new() ->DefAsyCry
    {
        let mut ignore_map = HashSet::new();
        let runtime = runtime::Builder::new_multi_thread()
            .worker_threads(6)
            .build()
            .unwrap();
        ignore_map.extend([9,
            EXT_SEND_FILE_CREATE,EXT_SEND_FILE,EXT_SEND_FILE_ELF,
            EXT_SAVE_FILE_CREATE,EXT_SAVE_FILE,EXT_SAVE_FILE_ELF,
            EXT_SAVE_FILE_RET,EXT_SAVE_FILE_CREATE_RET,EXT_SAVE_FILE_ELF_RET,
            EXT_UPLOAD_FILE_CREATE,EXT_UPLOAD_FILE,EXT_UPLOAD_FILE_ELF].iter());
        DefAsyCry{
            pri_key:None,
            pub_key:None,
            oth_ready:false,
            state:10,
            ignore_map,
            rt:Cell::new(Some(runtime))
        }
    }

    pub fn with_thread_count(thread_count:usize) ->DefAsyCry
    {
        let mut ignore_map = HashSet::new();
        let runtime = runtime::Builder::new_multi_thread()
            .worker_threads(thread_count)
            .build()
            .unwrap();
        ignore_map.extend([9,
            EXT_SEND_FILE_CREATE,EXT_SEND_FILE,EXT_SEND_FILE_ELF,
            EXT_SAVE_FILE_CREATE,EXT_SAVE_FILE,EXT_SAVE_FILE_ELF,
            EXT_SAVE_FILE_RET,EXT_SAVE_FILE_CREATE_RET,EXT_SAVE_FILE_ELF_RET,
            EXT_UPLOAD_FILE_CREATE,EXT_UPLOAD_FILE,EXT_UPLOAD_FILE_ELF].iter());
        DefAsyCry{
            pri_key:None,
            pub_key:None,
            oth_ready:false,
            state:10,
            ignore_map,
            rt:Cell::new(Some(runtime))
        }
    }

    pub fn ignore(&self,ext:u32)->bool
    {
        self.ignore_map.contains(&ext)
    }
}
#[async_trait]
impl AsyCry for DefAsyCry{

    async fn build_pub_key(&mut self) -> Result<Vec<u8>,u32> {

        if let Ok(key)= self.rt.get_mut().as_ref().unwrap().spawn(Self::real_build_pub_key(4096)).await
        {
            match key {
                Ok((k_data,k)) => {
                    self.pri_key = k;
                    Ok(k_data)
                }
                Err(e) => {
                    Err(e)
                }
            }
        }else{
            Err(EXT_ASY_CRY_ERR_GEN_KEY_UNKNOW)
        }
    }

    async fn real_build_pub_key(bit_size:usize) -> Result<(Vec<u8>,Option<RSAPrivateKey>),u32>
    {
        let mut rng = rand::rngs::OsRng;
        let priv_key = RSAPrivateKey::new(&mut rng, bit_size);
        if priv_key.is_err()
        {
            return Err(21); //21 gen private key failed
        }
        let pri_key = priv_key.ok();
        let pk = RSAPublicKey::from(pri_key.as_ref().unwrap());
        let mut n_bs = pk.n().to_bytes_be();
        let mut l_bs = pk.e().to_bytes_be();

        let n_bs_len = (n_bs.len() as u32).to_be_bytes();
        let l_bs_len = (l_bs.len() as u32).to_be_bytes();

        let mut res = vec![];
        n_bs_len.iter().for_each(|it|{
            res.push(*it)
        });
        res.append(&mut n_bs);
        l_bs_len.iter().for_each(|it|{
            res.push(*it)
        });
        res.append(&mut l_bs);
        Ok((res,pri_key))
    }

    fn pub_key_from(&self, d: &[u8], ext: u32) -> Result<RSAPublicKey, u32> {
        let n_l = u32_form_bytes(&d[0..4]);
        if n_l == 0 {
            return Err(22); //22 bad public key msg
        }
        let eb = 4 + n_l;
        if d.len() < eb as usize{
            dbg!(eb);
            return Err(22); //22 bad public key msg
        }
        let e_l = u32_form_bytes(&d[(eb as usize)..(eb as usize +4)]);
        if e_l == 0 {
            dbg!(e_l);
            return Err(22); //22 bad public key msg
        }
        if d.len() < (eb + 4 + e_l) as usize
        {
            dbg!(eb + 4 + e_l);
            return Err(22); //22 bad public key msg
        }
        let n = BigUint::from_bytes_be(&d[4usize..(n_l as usize + 4)]);
        let e = BigUint::from_bytes_be(&d[(eb as usize +4)..]);

        let pub_key = RSAPublicKey::new(n,e);
        if pub_key.is_err()
        {
            return Err(23); //22 bad public key
        }
        Ok(pub_key.unwrap())
    }


    ///ext
    async fn try_decrypt(&mut self, d: &[u8], ext: u32) -> EncryptRes {
        if self.ignore(ext) {
            EncryptRes::NotChange
        }else {
            if ext == 10
            {
                self.state = ext;
                let remote_pk = self.pub_key_from(d,ext);
                match remote_pk {
                    Ok(pk) => {
                        self.pub_key = Some(pk);
                    }
                    Err(e) => {
                        return EncryptRes::ErrMsg((vec![],e));
                    }
                }
                let pk = self.build_pub_key().await;
                match pk {
                    Ok(d) => {
                        self.state = 11;
                        return EncryptRes::RPubKey((d,11));
                    }
                    Err(e) => {
                        return EncryptRes::ErrMsg((vec![],e));
                    }
                }
            }else if ext == 11
            {
                if self.state != ext - 1{
                    dbg!(self.state);
                    return EncryptRes::ErrMsg((vec![],32));// 消息次序错误
                }
                self.state = ext;
                let remote_pk = self.pub_key_from(d,ext);
                match remote_pk {
                    Ok(pk) => {
                        self.pub_key = Some(pk);
                        self.state = 12;
                        return EncryptRes::ErrMsg((vec![],12));
                    }
                    Err(e) => {
                        return EncryptRes::ErrMsg((vec![],e));
                    }
                }
            }else if ext == 12 {
                if self.state != ext - 1{
                    return EncryptRes::ErrMsg((vec![],32));// 消息次序错误
                }
                self.state = ext;
                self.oth_ready = true;
                self.state = 13;
                return EncryptRes::ErrMsg((vec![],13));
            }else if ext == 13 {
                if self.state != ext - 1{
                    return EncryptRes::ErrMsg((vec![],32));// 消息次序错误
                }
                self.state = ext;
                self.oth_ready = true;
                return EncryptRes::Break;
            }else if ext >= 30 && ext < 40 {
                println!("Encrypt err msg code = {}",ext);
                return EncryptRes::Break;
            }
            if self.can_encrypt()
            {
                if let Some(ref k) = self.pri_key
                {
                    match k.decrypt(PaddingScheme::new_pkcs1v15_encrypt(), &d)
                    {
                        Ok(v) => {
                            EncryptRes::EncryptSucc(v)
                        }
                        Err(e) => {
                            EncryptRes::ErrMsg((vec![],30))
                        }
                    }
                }else {
                    EncryptRes::ErrMsg((vec![],31))
                }
            }else {
                EncryptRes::ErrMsg((vec![],31))
            }
        }
    }

    fn encrypt(&self, d: &[u8], ext: u32) -> EncryptRes {
        if self.ignore(ext) || (ext >= 10 && ext <= 20)
        {
            EncryptRes::NotChange
        }else{
            if let Some(ref k) = self.pub_key{
                let mut rng = rand::rngs::OsRng;
                let enc_data = k.encrypt(&mut rng, PaddingScheme::new_pkcs1v15_encrypt(),
                                         d);
                match enc_data {
                    Ok(ed) => {
                        EncryptRes::EncryptSucc(ed)
                    }
                    Err(ref e) => {
                        EncryptRes::ErrMsg((vec![],30))
                    }
                }
            }else{
                EncryptRes::NotChange
            }
        }
    }

    fn can_encrypt(&self) -> bool {
        self.oth_ready && self.pub_key.is_some() && self.pri_key.is_some()
    }
}

impl Drop for DefAsyCry{
    fn drop(&mut self) {
        let rt = self.rt.replace(None);
        rt.unwrap().shutdown_background();
    }
}