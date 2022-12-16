use std::sync::{Arc, Mutex};
use std::thread::ThreadId;
use jni::{AttachGuard, JNIEnv, sys};
use jni::objects::{JClass, JValue, GlobalRef, JObject};
use jni::sys::{JavaVM, jclass};
use static_init::{dynamic};

#[dynamic]
pub static GLOB_CXT: Arc<Mutex<Context>> = Arc::new(Mutex::new(Context::new()));

struct Ptr(usize);
impl Ptr{
    pub fn addr<T>(&self)->*const T
    {
        unsafe { std::mem::transmute::<usize,*const T>(self.0) }
    }
    pub fn addr_mut<T>(&self)->*mut T
    {
        unsafe { std::mem::transmute::<usize,*mut T>(self.0) }
    }
}
impl<T> From<*mut T> for Ptr{
    fn from(a: *mut T) -> Self {
        unsafe {Ptr(std::mem::transmute::<*mut T,usize>(a))}
    }
}
impl<T> From<*const T> for Ptr{
    fn from(a: *const T) -> Self {
        unsafe {Ptr(std::mem::transmute::<*const T,usize>(a))}
    }
}

pub enum LogLevel{
    Verbose,
    Debug,
    Info,
    Warn,
    Error,
    Assert
}

pub struct Context{
    jvm:Option<jni::JavaVM>,
    cxt:Option<GlobalRef>,
    agent:Option<GlobalRef>,
    main_thread_id:Option<std::thread::ThreadId>
}

impl Context {
    pub fn new()->Context
    {
        Context{
            jvm:None,
            cxt:None,
            agent:None,
            main_thread_id:None
        }
    }
    pub fn reg(&mut self,env:JNIEnv,cxt:JClass,agent:JClass)
    {
        let jvm = env.get_java_vm().unwrap();
        self.jvm = Some(jvm);   
        if let Ok(c) = env.new_global_ref(cxt){
            self.cxt = Some(c);
        }
        if let Ok(c) = env.new_global_ref(agent){
            self.agent = Some(c);
        }
        self.main_thread_id = Some(std::thread::current().id());
    }
    pub fn unreg(&mut self)
    {
        self.jvm = None;
        self.cxt = None;
        self.agent = None;
        self.main_thread_id = Some(std::thread::current().id());
    }
    pub unsafe fn call_method<'a>(&'a self,o:&'a GlobalRef,name:&str,sign:&str,args:&[JValue]) -> jni::errors::Result<JValue>
    {
        let jvm = self.get_jvm()?;
        let env = jvm.attach_current_thread()?;
        env.call_method(o.as_obj(),name,sign,args)
    }
    pub unsafe fn call_static_method(&self,cls:&str,name:&str,sign:&str,args:&[JValue]) -> jni::errors::Result<JValue>
    {
        let jvm = self.get_jvm()?;
        let env = jvm.attach_current_thread()?;
        let class = env.find_class(cls)?;
        env.call_static_method(class,name,sign,args)
    }
    pub unsafe fn call_agent_method(&self,name:&str,sign:&str,args:&[JValue]) -> jni::errors::Result<JValue>
    {
        if let Some(ref a) = self.agent{
            self.call_method(a,name,sign,args)
        }else { 
            Err(jni::errors::Error::NullPtr("May not registe!!!"))
        }
    }
    pub fn get_agent(&self) -> jni::errors::Result<JObject>
    {
        if let Some(ref a) = self.agent{
            Ok(a.as_obj())
        }else {
            Err(jni::errors::Error::NullPtr("May not registe!!!"))
        }
    }
    pub fn get_cxt(&self) -> jni::errors::Result<JObject>
    {
        if let Some(ref a) = self.cxt{
            Ok(a.as_obj())
        }else {
            Err(jni::errors::Error::NullPtr("May not registe!!!"))
        }
    }
    pub fn get_jvm(&self) -> jni::errors::Result<&jni::JavaVM>
    {
        return if let Some(ref jvm) = self.jvm
        {
            Ok(jvm)
        }else {
            Err(jni::errors::Error::NullPtr("jvm is null! are you regist?"))
        }
    }
    pub fn is_reg_thread(&self) -> bool
    {
        if let Some(id) = self.main_thread_id
        {
            return std::thread::current().id() == id;
        }
        return false;
    }
    pub fn log(&self,lvl:LogLevel,str:&str,tag:&str) -> jni::errors::Result<()>
    {
        let name = match lvl {
            LogLevel::Verbose => {"v"}
            LogLevel::Debug => {"d"}
            LogLevel::Info => {"i"}
            LogLevel::Warn => {"w"}
            LogLevel::Error => {"e"}
            LogLevel::Assert => {"wtf"}
        };
        let jvm = self.get_jvm()?;
        let env = jvm.attach_current_thread()?;
        let msg = env.new_string(str)?;
        let t = env.new_string(tag)?;
        unsafe { self.call_static_method("android/util/Log", name, "(Ljava/lang/String;Ljava/lang/String;)I", &[t.into(), msg.into()])?; }
        Ok(())
    }
    pub fn toast(&self,str:&str,op:i32) -> jni::errors::Result<()>
    {
        let jvm = self.get_jvm()?;
        let env = jvm.attach_current_thread()?;
        let log = env.new_string(str)?;
        let op = op as jni::sys::jint;
        unsafe {self.call_agent_method("toast","(Ljava/lang/String;I)V",&[log.into(),op.into()])?};
        Ok(())
    }
    pub fn on_err(&self,err:u32) -> jni::errors::Result<()>
    {
        let jvm = self.get_jvm()?;
        let env = jvm.attach_current_thread()?;
        let e = err as jni::sys::jint;
        unsafe {self.call_agent_method("get_error","(I)V",&[e.into()])?};
        Ok(())
    }
}