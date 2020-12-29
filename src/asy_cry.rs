
// ext 9        heartbeat
// ext 0        default
// ext 10 .. 20  asymmetric cryptographic
// ext 20 .. 40  asymmetric cryptographic err
// ext 40 41      upload file

use rsa::{PublicKey, RSAPrivateKey, PaddingScheme, RSAPublicKey, PublicKeyParts, BigUint};
use rsa::errors::Error;
use crate::tools::*;

#[derive(Debug)]
pub enum EncryptRes {
    EncryptSucc(Vec<u8>),
    RPubKey((Vec<u8>, u32)),
    ErrMsg((Vec<u8>, u32)),
    NotChange,
    Break,
}

pub trait AsyCry{

    fn build_pub_key(&mut self)->Result<Vec<u8>,u32>
    {
        Err(20) // 20 not impl
    }

    fn pub_key_from(&self,d:&[u8],ext:u32) ->Result<RSAPublicKey,u32>
    {
        Err(20)
    }

    fn try_decrypt(&mut self, d:&[u8],ext:u32) -> EncryptRes {
        EncryptRes::NotChange
    }

    fn encrypt(&self,d:&Vec<u8>,ext:u32) -> EncryptRes {
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
    oth_ready:bool,
    state:u32
}

impl DefAsyCry
{
    pub fn  new() ->DefAsyCry
    {
        DefAsyCry{
            pri_key:None,
            pub_key:None,
            oth_ready:false,
            state:10
        }
    }
}

impl AsyCry for DefAsyCry{

    fn build_pub_key(&mut self) -> Result<Vec<u8>,u32> {
        let mut rng = rand::rngs::OsRng;
        let priv_key = RSAPrivateKey::new(&mut rng, 2048);
        if priv_key.is_err()
        {
            return Err(21); //21 gen private key failed
        }
        self.pri_key = priv_key.ok();
        let pk = RSAPublicKey::from(self.pri_key.as_ref().unwrap());
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
        Ok(res)
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
    fn try_decrypt(&mut self, d: &[u8], ext: u32) -> EncryptRes {
        if ext == 9 {
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
                let pk = self.build_pub_key();
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

    fn encrypt(&self, d: &Vec<u8>, ext: u32) -> EncryptRes {
        if ext == 9 || (ext >= 10 && ext <= 20)
        {
            EncryptRes::NotChange
        }else{
            if let Some(ref k) = self.pub_key{
                let mut rng = rand::rngs::OsRng;
                let enc_data = k.encrypt(&mut rng, PaddingScheme::new_pkcs1v15_encrypt(),
                                         d.as_slice());
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