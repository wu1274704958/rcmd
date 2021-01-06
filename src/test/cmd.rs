
use std::process::{Command, Output, Stdio};
use tokio::io::Error;
use std::io::{Stdout, Stderr, Write, Read, stdin};
use std::thread::sleep;
use tokio::time::Duration;

use std::path::PathBuf;
use serde::{Serialize,Deserialize};
#[derive(Debug)]
pub struct Rcmd{
    curr_dir:PathBuf
}

#[derive(Debug,Deserialize,Serialize)]
pub struct CmdRes{
    pub out:String,
    pub err:String,
    pub code:Option<i32>,
}

impl Rcmd{
    pub fn new()->Rcmd
    {
        PathBuf::from(".");

        Rcmd{curr_dir:std::env::current_dir().unwrap()}
    }

    pub fn cd(&mut self,d:&str)->bool
    {
        if d.trim() == ".." {
            if self.curr_dir.parent().is_none(){
                return false;
            }
            return self.curr_dir.pop();
        }
        if d.trim() == "." {
            return true;
        }
        let mut n = self.curr_dir.clone();
        n.push(d.trim());
        if n.exists() && n.is_dir() {
            self.curr_dir = n;
            return true;
        }
        false
    }

    pub fn exec(&self,d:&str,args:&[&str])-> Result<CmdRes,String>
    {
        dbg!(&d);
        dbg!(&args);
        let mut c = match Command::new(d.trim())
            .args(args)
            .current_dir(self.curr_dir.to_str().unwrap())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn(){
            Ok(v) => {
                v
            }
            Err(e) => {
                return Err(format!("{:?}",e));
            }
        };

        let st = {
            match c.wait(){
                Ok(v) => {
                    v.code()
                }
                Err(e) => {
                    return Err(format!("{:?}",e));
                }
            }
        };

        let mut out = String::new();
        c.stdout.unwrap().read_to_string(&mut out);
        let mut err = String::new();
        c.stderr.unwrap().read_to_string(&mut err);

        Ok(CmdRes{
            code:st,
            out,err
        })
    }

    pub fn exec_ex(&mut self,s:String) ->Result<CmdRes,String>
    {
        let mut a:Vec<_> = s.split(" ").collect();
        a = a.iter_mut().map(|it|{
            it.trim()
        }).collect();
        return match a[0] {
            "cd" => {
                let v = self.cd(a[1]);
                Ok(CmdRes{
                    code:Some(if v {0}else{1}),
                    out:self.curr_dir.to_string_lossy().to_string(),
                    err:String::new()
                })
            }
            _ => {
                self.exec(a[0],&a[1..])
            }
        };
    }
}

fn main()
{

    let mut r = Rcmd::new();
    loop{
        let mut l = String::new();
        stdin().read_line(&mut l);
        dbg!(r.exec_ex(l));
    }
}