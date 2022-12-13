use std::sync::{Arc, Mutex};
use jni::{AttachGuard, JNIEnv, sys};
use jni::objects::{JClass, JValue};
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
    env:Ptr,
    cxt:Ptr,
    agent:Ptr,
    main_thread_id:std::thread::ThreadId
}

impl Context {
    pub fn new()->Context
    {
        Context{
            jvm:None,
            env:Ptr(0),
            cxt:Ptr(0),
            agent:Ptr(0),
            main_thread_id:std::thread::current().id()
        }
    }
    pub fn reg(&mut self,env:JNIEnv,cxt:JClass,agent:JClass)
    {
        let jvm = env.get_java_vm().unwrap();
        self.jvm = Some(jvm);   
        self.env = env.get_native_interface().into();
        self.cxt = cxt.into_raw().into();
        self.agent = agent.into_raw().into();
        self.main_thread_id = std::thread::current().id();
    }
    pub fn unreg(&mut self)
    {
        self.env = Ptr(0);
        self.cxt = Ptr(0);
        self.agent = Ptr(0);
        self.main_thread_id = std::thread::current().id();
    }
    pub unsafe fn call_method(&self,o:jclass,name:&str,sign:&str,args:&[JValue]) -> jni::errors::Result<JValue>
    {
        if o.is_null() {return Err(jni::errors::Error::NullPtr("object is null"));}
        let jvm = self.get_jvm()?;
        let env = jvm.attach_current_thread()?;
        let obj = JClass::from_raw(o);
        env.call_method(obj,name,sign,args)
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
        self.call_method(self.agent.addr_mut(),name,sign,args)
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
        std::thread::current().id() == self.main_thread_id
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
}