use std::env::args;
use mysql::{Pool, Value, PooledConn};
use mysql::prelude::Queryable;
use std::ffi::CStr;
use std::str::FromStr;

pub struct DBMgr{
    pool:Pool
}

impl DBMgr{
    pub fn new()->Option<DBMgr>
    {
        let mut pwd = "as147258369".to_string();
        let mut prot = 3306;
        args().enumerate().for_each(|it|{
            match it.0{
                3 => {
                    pwd = it.1;
                }
                4 => {
                     if let Ok(v) = i32::from_str( it.1.as_str())
                     {
                         prot = v;
                     }
                }
                _=>{}
            }
        });

        let url = format!("mysql://root:{}@localhost:{}/sql_test",pwd,prot);
        let pool = match Pool::new(url){
            Ok(p) => {
                p
            }
            Err(e) => {
                dbg!(e);
                return None;
            }
        };

        let mut conn = pool.get_conn().unwrap();

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
            create table user(id BIGINT NOT NULL primary key auto_increment,\
                name varchar(255) not null,\
                acc varchar(255) not null,\
                pwd varchar(255) not null,\
                is_admin boolean DEFAULT FALSE,\
                super_admin boolean DEFAULT FALSE );", ()).unwrap();

            conn.exec_drop("insert into user (name,acc,pwd,is_admin,super_admin) values((?),(?),(?),(?),(?));",
                           ("wws","wws","31726",true,true)).unwrap();
        }

        Some(DBMgr{pool})
    }

    pub fn get_conn(&self) -> Result<PooledConn,mysql::Error>
    {
        self.pool.get_conn()
    }
}