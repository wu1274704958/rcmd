mod context;
mod client;
mod model;
mod client_plugs;
use jni::JNIEnv;
use jni::objects::{JClass, JString};
use tokio::runtime::Runtime;
use std::sync::Arc;
use std::{ thread, time::Duration};
use context::GLOB_CXT;
use crate::context::LogLevel;
use client::scl::NetContext;
use client::scl;

// This keeps rust from "mangling" the name and making it unique for this crate.
#[no_mangle]
pub extern "system" fn Java_com_wws_remotecamad_RemoteCamAgent_registe_1low(
    env: JNIEnv,
    _class: JClass,
    cxt: JClass,
){
    if let Ok(mut c) = GLOB_CXT.lock()
    {
        c.reg(env,cxt,_class);
        c.toast("Registe success!!!",1).unwrap();
    }
}

pub fn launch(args:String)
{
    toast( format!("launch {}",args),0);
    let runtime = Runtime::new().unwrap();
    let cxt = Arc::new(tokio::sync::Mutex::new(NetContext::new()));
    let res = runtime.spawn(launch_real(args,cxt));
    while !res.is_finished() {
        thread::sleep(Duration::from_secs(1));
    }
    toast("scl thread end!!!".to_string(), 0);
}

async fn launch_real(args: String, cxt:Arc<tokio::sync::Mutex<NetContext>>) -> std::io::Result<()>
{
    let run_res = scl::run(args,cxt).await;
    toast(format!("{:?}",run_res), 0);
    Ok(())
}

#[no_mangle]
pub extern "system" fn Java_com_wws_remotecamad_RemoteCamAgent_unregiste_1low(
    _env: JNIEnv,
    _class: JClass
){
    if let Ok(mut c) = GLOB_CXT.lock()
    {
        c.unreg();
    }
}

#[no_mangle]
pub extern "system" fn Java_com_wws_remotecamad_RemoteCamAgent_launch(
    _env: JNIEnv,
    _class: JClass,
    args: JString
){
    if let Ok(mut c) = GLOB_CXT.lock()
    {
        let jvm = c.get_jvm().unwrap();
        let env = jvm.attach_current_thread().unwrap();
        let args:String = env.get_string(args).unwrap().into();
        let test = std::thread::spawn(||{ launch(args)});
    }
}

fn toast(s:String,p:i32)
{
    if let Ok(mut c) = GLOB_CXT.lock()
    {
        c.toast(s.as_str(), p).unwrap();
    }
}