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

#[cfg(feature = "mysql")]
use mysql::prelude::FromRow;
#[cfg(feature = "mysql")]
use mysql::{Row, Value, FromRowError};

#[cfg(feature = "mysql")]
impl FromRow for User
{
    fn from_row(row: Row) -> Self where
        Self: Sized, {
        let mut u = User::default();
        row.columns().iter().enumerate().for_each(|(i,c)|{
            match c.name_str().as_ref() {
                "id" => {
                    if let Some(Value::Bytes(v)) = row.get(i){
                        if let Ok(i ) = u64::from_str(String::from_utf8_lossy(v.as_slice()).as_ref())
                        {
                            u.id = i;
                        }
                    }
                }
                "name" => {
                    if let Some(Value::Bytes(v)) = row.get(i){
                        u.name = String::from_utf8_lossy(v.as_slice()).to_string();
                    }
                }
                "acc" => {
                    if let Some(Value::Bytes(v)) = row.get(i){
                        u.acc = String::from_utf8_lossy(v.as_slice()).to_string();
                    }
                }
                "pwd" => {
                    if let Some(Value::Bytes(v)) = row.get(i){
                        u.pwd = String::from_utf8_lossy(v.as_slice()).to_string();
                    }
                }
                "is_admin" => {
                    if let Some(Value::Bytes(v)) = row.get(i){
                        if let Ok(i ) = u32::from_str(String::from_utf8_lossy(v.as_slice()).as_ref())
                        {
                            u.is_admin = i == 1;
                        }
                    }
                }
                "super_admin" => {
                    if let Some(Value::Bytes(v)) = row.get(i){
                        if let Ok(i ) = u32::from_str(String::from_utf8_lossy(v.as_slice()).as_ref())
                        {
                            u.super_admin = i == 1;
                        }
                    }
                }
                _ =>{

                }
            }
        });
        u
    }

    fn from_row_opt(row: Row) -> std::result::Result<Self, FromRowError> where
        Self: Sized {
        let u = Self::from_row(row.clone());
        if u.id > 0{
            Ok(u)
        }else{
            Err(mysql::FromRowError(row))
        }
    }
}


