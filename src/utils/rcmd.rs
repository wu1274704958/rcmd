use std::path::PathBuf;
use std::process::{Command, Stdio, ExitStatus};
use std::io::{Read, Write};
use serde::{
    Serialize,
    Deserialize
};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use tokio::io::Error;
use tokio::stream::StreamExt;
use std::fs::OpenOptions;
use subprocess::{Exec, Redirection};
use encoding_rs::GBK;

#[derive(Debug)]
pub struct Rcmd{
    curr_dir:PathBuf,
    pub timeout:Option<Duration>
}

#[derive(Debug,Deserialize,Serialize)]
pub struct CmdRes{
    pub out:String,
    pub err:String,
    pub code:Option<i32>,
}

impl CmdRes{
    pub fn err_str(s:String)->CmdRes
    {
        CmdRes {
            out : "".to_string(),
            err : s,
            code : Some(-1)
        }
    }
    pub fn new(out:String,err:String,code:Option<i32>)->CmdRes
    {
        CmdRes {
            out ,
            err ,
            code
        }
    }
}

impl Rcmd{
    pub fn new(timeout:Option<Duration>)->Rcmd
    {
        PathBuf::from(".");

        Rcmd{curr_dir:std::env::current_dir().unwrap(),
            timeout
        }
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
        let st = match self.timeout{
            None => {
                match c.wait(){
                    Ok(v) => {
                        v.code()
                    }
                    Err(e) => {
                        return Err(format!("{:?}",e));
                    }
                }
            }
            Some(dur) => {
                let b = SystemTime::now();
                loop {
                    match c.try_wait() {
                        Ok(Some(status)) => {
                            break status.code();
                        }
                        Ok(None) => {

                        }
                        Err(e) => {
                            return Err(format!( "error attempting to wait: {}",e));
                        }
                    }
                    if let Ok(nd) = SystemTime::now().duration_since(b)
                    {
                        if nd > dur{
                            return Err(format!("time out {} ",d));
                        }
                    }
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
            let mut exec = match Exec::cmd("C:\\Windows\\System32\\cmd.exe")
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
                    println!("write.............");
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
        let out = match OpenOptions::new().read(true).open(out_file){
            Ok(mut o) => {
                let mut vec = vec![];
                o.read_to_end(&mut vec);
                drop(o);
                std::fs::remove_file(out_file);
                let mut decoder = encoding_rs::GBK.new_decoder();
                if let Some(len) = decoder.max_utf8_buffer_length(vec.len()){
                    let mut s = String::with_capacity(len);
                    decoder.decode_to_string(vec.as_slice(),&mut s,false);
                    s
                }else{
                    return Err(format!("Decode out result get max buffer len failed!!!"));
                }
            }
            Err(e) => {return Err(format!("open stdout failed {:?}",e));}
        };
        let err = match OpenOptions::new().read(true).open(err_file){
            Ok(mut o) => {
                let mut vec = vec![];
                o.read_to_end(&mut vec);
                drop(o);
                std::fs::remove_file(err_file);
                match std::str::from_utf8(vec.as_slice()){
                    Ok(s) => {s.to_string()}
                    Err(_) => {
                        let mut decoder = encoding_rs::GBK.new_decoder();
                        if let Some(len) = decoder.max_utf8_buffer_length(vec.len()){
                            let mut s = String::with_capacity(len);
                            decoder.decode_to_string(vec.as_slice(),&mut s,false);
                            s
                        }else{
                            return Err(format!("Decode out result get max buffer len failed!!!"));
                        }
                    }
                }
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