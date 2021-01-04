use mysql::prelude::FromRow;
use mysql::{Row, Value, FromRowError};
use std::str::FromStr;
use crate::model::user::User;

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