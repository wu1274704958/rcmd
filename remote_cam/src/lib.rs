mod context;
mod client;
mod model;
use jni::JNIEnv;
use jni::objects::{GlobalRef, JClass, JObject, JString};
use jni::sys::{jbyteArray, jint, jlong, jstring};
use std::{sync::mpsc, thread, time::Duration};
use context::GLOB_CXT;
use crate::context::LogLevel;

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
        c.log(LogLevel::Info,"launch thread","@v@").unwrap();
    }
    let test = std::thread::spawn(test);
}

pub fn test()
{
    thread::sleep(Duration::from_secs(2));
    if let Ok(c) = GLOB_CXT.lock(){
        c.log(LogLevel::Error,"Log in thread","@v@");
        c.toast("toast in thread",0).unwrap();
    }
    thread::sleep(Duration::from_secs(2));
    if let Ok(c) = GLOB_CXT.lock(){
        c.on_err(0);
    }
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