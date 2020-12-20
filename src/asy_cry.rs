
// ext 9        heartbeat
// ext 0        default
// ext 10 - 20  asymmetric cryptographic


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

}

impl DefAsyCry
{
    pub fn  new() ->DefAsyCry
    {
        DefAsyCry{}
    }
}

impl AsyCry for DefAsyCry{

}