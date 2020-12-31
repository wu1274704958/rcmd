

use mysql::*;
use mysql::prelude::*;
use std::env::args;
use mysql::uuid::Bytes;
use std::ffi::CStr;
use std::fs::read;
use std::borrow::Cow;
use std::str::FromStr;

#[derive(Clone,Debug,Eq, PartialEq)]
pub struct User
{
    pub id:u64,
    pub name:String,
    pub acc:String,
    pub pwd:String,
    pub is_admin:bool,
    pub super_admin:bool
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
        if u.id == 0{
            Ok(u)
        }else{
            Err(mysql::FromRowError(row))
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Payment {
    customer_id: i32,
    amount: i32,
    account_name: Option<String>,
}
fn main()
{
    let mut pwd = "as147258369".to_string();

    args().enumerate().for_each(|it|{
        if it.0 == 1{
            pwd = it.1;
        }
    });

    let url = format!("mysql://root:{}@localhost:3306/sql_test",pwd);

    let pool = match Pool::new(url){
        Ok(p) => {
            p
        }
        Err(e) => {
            dbg!(e);
            return;
        }
    };

    let mut conn = pool.get_conn().unwrap();

// Let's create a table for payments.
    let mut has_table = false;
    {
        let mut result = conn.query_iter("show tables;").unwrap();
        has_table =
            result.find(|it| {
                if let Ok(r) = it {
                    if let Some(Value::Bytes(mut b)) = r.get::<Value, usize>(0) {
                        b.push(b'\0');
                        let a = CStr::from_bytes_with_nul(b.as_slice()).unwrap();
                        if a == CStr::from_bytes_with_nul(b"user\0").unwrap() { return true; }
                    }
                }
                false
            }).is_some();
    }


    if !has_table {
        println!("has_table = {}, exec table user", has_table);
        conn.exec_drop("\
        create table User(id BIGINT NOT NULL primary key auto_increment,\
         name varchar(255) not null,\
         acc varchar(255) not null,\
         pwd varchar(255) not null,\
         is_admin boolean DEFAULT FALSE,\
         super_admin boolean DEFAULT FALSE );", ()).unwrap();

        conn.exec_drop("insert into user (name,acc,pwd,is_admin,super_admin) values((?),(?),(?),(?),(?));",
                       ("wws","wws","31726",true,true)).unwrap();
    }

    let sel:Result<Option<User>> = conn.query_first::<User,_>("select * from user where user.name='wws'");

    println!("Yay!");
}