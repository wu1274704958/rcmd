
use std::process::{Command, Output, Stdio};
use tokio::io::Error;
use std::io::{Stdout, Stderr, Write, Read, stdin};
use std::thread::sleep;
use tokio::time::Duration;

use std::path::PathBuf;
use serde::{Serialize,Deserialize};
use std::collections::HashMap;
use std::fs::{OpenOptions, File};
use subprocess::{Exec, PopenError, ExitStatus, Popen, Redirection};


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
    #[cfg(target_os = "windows")]
    fn append_env(v:&mut String,s:&str)
    {
        v.push_str(s);
        v.push(';');
    }

    #[cfg(not(target_os = "windows"))]
    fn append_env(v:&mut String,s:&str)
    {
        v.push(':');
        v.push_str(s);
        v.push(':');
    }
    #[cfg(not(target_os = "windows"))]
    pub fn exec(&self,d:&str,args:&[&str])-> Result<CmdRes,String>
    {

        dbg!(&d);
        dbg!(&args);

        let mut filtered_env : HashMap<String, String> = std::env::vars().collect();
        if let Some(p) = filtered_env.get_mut(&"PATH".to_string()){
            Self::append_env(p,".");
            dbg!(&p);
        };

        let mut c = match Command::new(d.trim())
            .args(args)
            .current_dir(self.curr_dir.to_str().unwrap())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .envs(&filtered_env)
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

    #[cfg(target_os = "windows")]
    pub fn exec(&self,d:&str,args:&[&str])-> Result<CmdRes,String>
    {
        dbg!(&d);
        dbg!(&args);

        let out_file ="exec_cmd_stdout.txt";
        let err_file ="exec_cmd_stderr.txt";

        let mut filtered_env : HashMap<String, String> = std::env::vars().collect();
        if let Some(p) = filtered_env.get_mut(&"PATH".to_string()){
            Self::append_env(p,".");
        };
        let f_out = match OpenOptions::new().create(true).write(true).append(false).open(out_file){
            Ok(o) => {o}
            Err(e) => {return Err(format!("open stdout failed {:?}",e));}
        };
        let f_err = match OpenOptions::new().create(true).write(true).append(false).open(err_file){
            Ok(o) => {o}
            Err(e) => {return Err(format!("open stderr failed {:?}",e));}
        };
        let st = {
            let mut exec = match Exec::cmd("cmd")
                .cwd(self.curr_dir.clone())
                .stdin(Redirection::Pipe)
                .stdout(Redirection::File(f_out))
                .stderr(Redirection::File(f_err))
                .popen()
            {
                Ok(e) => { e }
                Err(e) => { return Err(format!("stdin failed ")); }
            };

            let in_ = exec.stdin.as_mut();

            match in_ {
                Some(mut f) => {
                    println!("write....");
                    f.write(d.as_bytes());
                    args.iter().for_each(|it| {
                        f.write(b" ".as_ref());
                        f.write((*it).as_bytes());
                    });
                    f.write(b"\r\n".as_ref());
                    f.write(b"exit\r\n".as_ref());
                    f.sync_data();
                    f.sync_all();
                }
                None => {
                    return Err(format!("stdin failed "));
                }
            }
            //exec.wait_timeout()
            let state = exec.wait();
             match state {
                Ok(o) => {
                    Some(0)
                }
                Err(e) => {
                    return Err(format!("Exec failed {:?}", e));
                }
            }
        };
        sleep(Duration::from_secs(1));
        let out = match OpenOptions::new().read(true).open(out_file){
            Ok(mut o) => {
                let mut vec = vec![];
                o.read_to_end(&mut vec);
                String::from_utf8_lossy(vec.as_slice()).trim().to_string()
            }
            Err(e) => {return Err(format!("open stdout failed {:?}",e));}
        };
        let err = match OpenOptions::new().read(true).open(err_file){
            Ok(mut o) => {
                let mut vec = vec![];
                o.read_to_end(&mut vec);
                String::from_utf8_lossy(vec.as_slice()).trim().to_string()
            }
            Err(e) => {return Err(format!("open stderr failed {:?}",e));}
        };

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
        let a = r.exec_ex(l).unwrap();
        println!("{}",a.out);
    }
}
