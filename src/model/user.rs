use std::str::FromStr;
use serde::{Serialize, Deserialize};

#[derive(Clone,Debug,PartialEq,Eq, Serialize, Deserialize)]
pub struct User
{
    pub id:u64,
    pub name:String,
    pub acc:String,
    pub pwd:String,
    pub is_admin:bool,
    pub super_admin:bool
}

#[derive(Clone,Debug,PartialEq,Eq, Serialize, Deserialize)]
pub struct MinUser
{
    pub acc:String,
    pub pwd:String,
}

#[derive(Clone,Debug,PartialEq,Eq, Serialize, Deserialize)]
pub struct RegUser
{
    pub name:String,
    pub acc:String,
    pub pwd:String,
}

#[derive(Clone,Debug,PartialEq,Eq, Serialize, Deserialize)]
pub struct GetUser
{
    pub lid:usize,
    pub name:String
}

impl Default for User
{
    fn default() -> Self {
        User{
            id: 0,
            name:  "".to_string(),
            acc:  "".to_string(),
            pwd:  "".to_string(),
            is_admin:  false,
            super_admin:  false,
        }
    }
}


