
// ext 9        heartbeat
// ext 0        default
// ext 10 - 20  asymmetric cryptographic


use rsa::{RSAPrivateKey, RSAPublicKey, PublicKey, PaddingScheme};
use rsa::errors::Error;

pub enum EncryptRes {
    EncryptSucc(Vec<u8>),
    RPubKey((Vec<u8>, u32)),
    ErrMsg((Vec<u8>, u32)),
    NotChange
}

pub trait AsyCry{
    fn build_pub_key(&mut self)->(Vec<u8>,u32)
    {
        (vec![0],0)
    }

    fn try_decrypt(&mut self, d:&[u8],ext:u32) -> EncryptRes {
        EncryptRes::NotChange
    }

    fn encrypt(&self,d:&Vec<u8>,ext:u32) -> EncryptRes {
        EncryptRes::NotChange
    }

    fn can_encrypt(&self)->bool
    {
        false
    }
}

pub struct DefAsyCry{
    pri_key: Option<RSAPrivateKey>,
    pub_key: Option<RSAPublicKey>,
    oth_ready:bool
}

impl DefAsyCry
{
    pub fn  new() ->DefAsyCry
    {
        DefAsyCry{
            pri_key:None,
            pub_key:None,
            oth_ready:false
        }
    }
}

impl AsyCry for DefAsyCry{
    fn build_pub_key(&mut self) -> (Vec<u8>, u32) {
        unimplemented!()
    }

    fn try_decrypt(&mut self, d: &[u8], ext: u32) -> EncryptRes {

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
                        EncryptRes::ErrMsg(e)
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