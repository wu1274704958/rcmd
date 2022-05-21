
use std::{collections::VecDeque, io, net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs}, sync::{Arc}, time::{Duration, SystemTime}};
use std::vec::Vec;
use tokio::{io::AsyncWriteExt, net::{UdpSocket}, time::sleep};
use tokio::sync::Mutex;
use crate::{agreement::{Agreement, Message}, asy_cry::{DefAsyCry,AsyCry,EncryptRes,NoAsyCry},  subpackage::{DefSubpackage,Subpackage}, utils::msg_split::{DefMsgSplit,MsgSplit}};
use crate::utils::udp_sender::{DefUdpSender,UdpSender,USErr};
use crate::tools::platform_handle;
use async_std::io::Error;
use async_std::future::Future;
use crate::client_handler::Handle;
use std::thread::Thread;
use crate::client_plug::client_plug::{ClientPlug,ClientPluCollect};
use std::io::Write;
use async_std::channel::{Receiver, RecvError};

pub enum UdpClientErr{
    UsError(USErr),
    NoUdpSender
}

impl From<USErr> for UdpClientErr {
    fn from(e: USErr) -> Self {
        UdpClientErr::UsError(e)
    }
}
impl From<std::io::Error> for UdpClientErr {
    fn from(e: Error) -> Self {
        UdpClientErr::UsError(e.into())
    }
}

pub struct UdpClient<T,A,SE>
    where T:Handle,
    SE : UdpSender + Send + std::marker::Sync
{
    msg_queue:Arc<Mutex<VecDeque<(Vec<u8>,u32)>>>,
    runing:Arc<Mutex<bool>>,
    handler:Arc<T>,
    sender:Arc<Mutex<Option<Arc<SE>>>>,
    pub heartbeat_dur:Duration,
    pub nomsg_rest_dur:Duration,
    bind_addr:SocketAddr,
    pub buf_size:usize,
    parser:A,
    local_addr:Arc<Mutex<Option<SocketAddr>>>
}

fn socket_addr_conv<T:ToSocketAddrs>(t:T) -> io::Result<SocketAddr>{
    t.to_socket_addrs()?.next().ok_or(io::Error::from(io::ErrorKind::InvalidInput))
}


macro_rules! Stop {
    ($S:ident,$RecvWorker:ident,$Plug:ident,$E:ident,$RET:ident) => {
        $Plug.on_get_err($E.clone()).await;
        $S.stop().await;
        $RecvWorker.shutdown_background();
        $Plug.on_stop().await;
        $RET.await;
        return Err($E.into());
    };
    ($S:ident,$Plug:ident,$E:ident,$RET:ident) => {
        $Plug.on_get_err($E.clone()).await;
        $S.stop().await;
        $Plug.on_stop().await;
        $RET.await;
        return Err($E.into());
    };
}

macro_rules! StopNoPlug {
    ($S:ident,$RecvWorker:ident,$E:ident,$RET:ident) => {
        $S.stop().await;
        $RecvWorker.shutdown_background();
        $RET.await;
        return Err($E.into());
    };
    ($S:ident,$E:ident,$RET:ident) => {
        $S.stop().await;
        $RET.await;
        return Err($E.into());
    };
}

impl <'a,T,A,SE> UdpClient<T,A,SE>
    where T:Handle + std::marker::Sync,
          A : Agreement,
          SE : UdpSender + Send + std::marker::Sync + 'static,
{
    pub fn new(bind_addr:impl ToSocketAddrs,handler:Arc<T>,parser:A)-> Self
    {
        UdpClient::<T,A,SE>{
            msg_queue : Arc::new(Mutex::new(VecDeque::new())),
            runing:Arc::new(Mutex::new(true)),
            handler,
            heartbeat_dur:Duration::from_secs(5),
            nomsg_rest_dur:Duration::from_millis(1),
            parser,
            bind_addr : socket_addr_conv(bind_addr).unwrap(),
            buf_size:1024 * 1024 * 10,
            local_addr: Arc::new(Mutex::new(None)),
            sender : Arc::new(Mutex::new(None))
        }
    }

    pub fn with_dur(bind_addr:impl ToSocketAddrs,handler:Arc<T>,parser:A,
                    heartbeat_dur:Duration,
                    nomsg_rest_dur:Duration)-> Self
    {
        UdpClient::<T,A,SE>{
            msg_queue : Arc::new(Mutex::new(VecDeque::new())),
            runing:Arc::new(Mutex::new(true)),
            handler,
            heartbeat_dur,
            nomsg_rest_dur,
            parser,
            bind_addr:socket_addr_conv(bind_addr).unwrap(),
            buf_size:1024 * 1024 * 10,
            local_addr: Arc::new(Mutex::new(None)),
            sender : Arc::new(Mutex::new(None))
        }
    }

    pub fn with_msg_queue(bind_addr:impl ToSocketAddrs,handler:Arc<T>,parser:A,
                          msg_queue:Arc<Mutex<VecDeque<(Vec<u8>,u32)>>>)-> Self
    {
        UdpClient::<T,A,SE>{
            msg_queue,
            runing:Arc::new(Mutex::new(true)),
            handler,
            heartbeat_dur:Duration::from_secs(5),
            nomsg_rest_dur:Duration::from_millis(1),
            parser,
            bind_addr: socket_addr_conv(bind_addr).unwrap(),
            buf_size:1024 * 1024 * 10,
            local_addr: Arc::new(Mutex::new(None)),
            sender : Arc::new(Mutex::new(None))
        }
    }

    pub fn with_msg_queue_runing(bind_addr:impl ToSocketAddrs,handler:Arc<T>,parser:A,
                                 msg_queue:Arc<Mutex<VecDeque<(Vec<u8>,u32)>>>,
                                 runing:Arc<Mutex<bool>>)-> Self
    {
        UdpClient::<T,A,SE>{
            msg_queue,
            runing,
            handler,
            heartbeat_dur:Duration::from_secs(5),
            nomsg_rest_dur:Duration::from_millis(1),
            parser,
            bind_addr: socket_addr_conv(bind_addr).unwrap(),
            buf_size:1024 * 1024 * 10,
            local_addr: Arc::new(Mutex::new(None)),
            sender : Arc::new(Mutex::new(None))
        }
    }

    pub async fn get_local_addr(&self,data: Vec<u8>,ext:u32) -> Option<SocketAddr> {
        let mut addr = self.local_addr.lock().await;
        *addr
    }

    pub async fn send(&self,data: Vec<u8>,ext:u32) {
        let mut a = self.msg_queue.lock().await;
        {
            a.push_back((data,ext));
        }
    }

    pub async fn stop(&self)
    {
        let mut b = self.runing.lock().await;
        *b = false;
    }

    pub async fn still_runing(&self)->bool
    {
        let b = self.runing.lock().await;
        *b
    }

    pub async fn build_session(&self)-> Result<(),UdpClientErr>
    {
        let sender = self.sender.lock().await;
        if let Some(se) = (*sender).clone()
        {
            se.build_session().await?
        }else{
            return Err(UdpClientErr::NoUdpSender);
        }
        Ok(())
    }

    pub async fn close_session(&self)-> Result<(),UdpClientErr>
    {
        let sender = self.sender.lock().await;
        if let Some(se) = (*sender).clone()
        {
            se.close_session().await?
        }else{
            return Err(UdpClientErr::NoUdpSender);
        }
        Ok(())
    }

    async fn pop_msg(&self)->Option<(Vec<u8>,u32)>
    {
        let mut queue = self.msg_queue.lock().await;
        queue.pop_front()
    }


    async fn write_msg(&self, data: Vec<u8>, ext:u32)->Result<(),USErr>
    {
        let sender = self.sender.lock().await;
        if let Some(se) = (*sender).clone()
        {
            se.send_msg(self.parser.package_nor(data,ext)).await?
        }else{
            eprintln!("call write_msg() No Sender!!!");
        }
        Ok(())
    }

    async fn set_sender(&self,se:Arc<SE>)
    {
        let mut sender = self.sender.lock().await;
        *sender = Some(se);
    }

    async fn clear_sender(&self)
    {
        let mut sender = self.sender.lock().await;
        *sender = None;
    }

    #[allow(unused_must_use)]
    pub async fn run<CP,F>(&self,ip:Ipv4Addr,port:u16,asy_cry_ignore:Option<&Vec<u32>>,
                     msg_split_ignore:Option<&Vec<u32>>,
                    plug_collect:Arc<ClientPluCollect<CP>>,
                    on_ret: F
    ) -> Result<(),UdpClientErr>

    where
        CP : ClientPlug<SockTy = UdpSocket,ErrTy = USErr> + Send + std::marker::Sync + 'static,
        <CP as ClientPlug>::SockTy: std::marker::Send + std::marker::Sync,
        F: futures::Future<Output=()>
    {
        plug_collect.on_init().await;

        let sock = Arc::new( UdpSocket::bind(self.bind_addr).await? );
        platform_handle(sock.as_ref());
        let addr = SocketAddr::new(IpAddr::V4(ip), port);


        plug_collect.on_create_socket(sock.clone()).await;

        if let Ok(addr) = sock.local_addr() {
            plug_collect.on_get_local_addr(addr).await;
            let mut local_addr = self.local_addr.lock().await;
            *local_addr = Some(addr);
        }

        //sock.connect(addr).await?;

        let mut buf = Vec::with_capacity(self.buf_size);
        buf.resize(self.buf_size,0);
        // In a loop, read data from the socket and write the data back.
        let mut heartbeat_t = SystemTime::now();
        let mut asy = DefAsyCry::create();
        if let Some(v) = asy_cry_ignore{
            asy.extend_ignore(v.as_slice());
        }
        let mut spliter = DefMsgSplit::new();
        if let Some(v) = msg_split_ignore{
            spliter.extend_ignore(v.as_slice());
        }
        let mut package = None;

        let sender = Arc::new(SE::create(sock.clone(),addr));
        let err:Arc<Mutex<Option<USErr>>> = Arc::new(Mutex::new(None));
        let sender_cp = sender.clone();
        let run_cp = self.runing.clone();
        let sock_cp = sock.clone();
        let err_cp = err.clone();
        let addr_cp = addr;
        let plugs_cp = plug_collect.clone();

        self.set_sender(sender.clone()).await;

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .build()
            .unwrap();

        plug_collect.on_lauch_recv_worker().await;
       let recv_worker = runtime.spawn(async move {
            let r = {
                let r = run_cp.lock().await;
                *r
            };
            'Out: while r {
                match sock_cp.recv_from(&mut buf).await {
                    Ok((len,addr)) => {
                        if addr == addr_cp{
                            match sender_cp.check_recv(&buf[0..len]).await
                            {
                                Err(USErr::EmptyMsg)=>{}
                                Err(e)=>{
                                    let mut err = err_cp.lock().await;
                                    *err = Some(e.into());
                                    let mut run = run_cp.lock().await;
                                    *run = false;
                                    break;
                                }
                                _=>{}
                            };
                            while sender_cp.need_check().await {
                                match sender_cp.check_recv(&[]).await{
                                    Err(USErr::EmptyMsg)=>{}
                                    Err(e)=>{
                                        let mut err = err_cp.lock().await;
                                        *err = Some(e.into());
                                        let mut run = run_cp.lock().await;
                                        *run = false;
                                        break 'Out;
                                    }
                                    _=>{}
                                }
                            }
                        }else{
                            plugs_cp.on_recv_oth_msg(addr,&buf[0..len]).await;
                        }
                    }
                    Err(e) => {
                        eprintln!("recv from {:?}",e);
                        let mut err = err_cp.lock().await;
                        *err = Some(e.into());
                        let mut run = run_cp.lock().await;
                        *run = false;
                        break;
                    }
                }
            }
        });


        if let Ok(pub_key_data) = asy.build_pub_key().await{
            if let Err(e) =  self.write_msg(pub_key_data, 10).await{
                Stop!(self,runtime,plug_collect,e,on_ret);
            }
        }

        let mut subpackager = DefSubpackage::new();

        plug_collect.on_lauch_loop().await;

        loop {
            // read request
            //println!("read the request....");
            {
                let err = err.lock().await;
                if let Some(e) = (*err).clone(){
                    Stop!(self,runtime,plug_collect,e,on_ret);
                }
            };
            match sender.pop_recv_msg().await{
                Ok(v) => {
                    package = subpackager.subpackage(&v[..],v.len());
                }
                Err(USErr::EmptyMsg) => {}
                Err(e) => {
                    Stop!(self,runtime,plug_collect,e,on_ret);
                }
            }

            if package.is_none() && subpackager.need_check(){
                package = subpackager.subpackage(&[],0);
            }

            if let Some( mut d) = package {
                package = None;
                let mut _temp_data = None;
                let msg = self.parser.parse_tf(&mut d);
                //dbg!(&msg);
                if let Some(mut m) = msg {
                    //----------------------------------
                    let mut immediate_send = None;
                    let mut override_msg = None;
                    match asy.try_decrypt(m.msg, m.ext).await
                    {
                        EncryptRes::EncryptSucc(d) => {
                            override_msg = Some(d);
                        }
                        EncryptRes::RPubKey(d) => {
                            immediate_send = Some(d.0);
                            m.ext = d.1;
                        }
                        EncryptRes::ErrMsg(d) => {
                            immediate_send = Some(d.0);
                            m.ext = d.1;
                        }
                        EncryptRes::NotChange => {}
                        EncryptRes::Break => { continue; }
                    };
                    if let Some(v) = immediate_send
                    {
                        if let Err(e) = self.write_msg( v, m.ext).await{
                            Stop!(self,runtime,plug_collect,e,on_ret);
                        }
                        continue;
                    }
                    if let Some(ref v) = override_msg
                    {
                        m.msg = v.as_slice();
                    }
                    // if m.ext != 9
                    // {println!("{:?} {}",&m.msg,m.ext);}
                    if spliter.need_merge(&m)
                    {
                        if let Some((data,ext)) = spliter.merge(&m)
                        {
                            _temp_data = Some(data);
                            m.ext = ext;
                            m.msg = _temp_data.as_ref().unwrap().as_slice();
                        }else{
                            continue;
                        }
                    }
                    plug_collect.handle(m).await;
                    if let Some((d,e)) = self.handler.handle_ex(m).await
                    {
                        self.send(d,e).await;
                    }
                }
                package = None;
            }
            if let Err(e) = sender.check_send().await{
                Stop!(self,runtime,plug_collect,e,on_ret);
            }

            if let Ok(n) = SystemTime::now().duration_since(heartbeat_t)
            {
                if n >= self.heartbeat_dur
                {
                    heartbeat_t = SystemTime::now();
                    if let Err(e) = self.write_msg(vec![9], 9).await{
                        Stop!(self,runtime,plug_collect,e,on_ret);
                    }
                }
            }

            if asy.can_encrypt() {
                let mut _data = self.pop_msg().await;
                if let Some(mut v) = _data {
                    if spliter.need_split(v.0.len(),v.1)
                    {
                        let msgs = spliter.split(&mut v.0,v.1);
                        for i in msgs.into_iter(){
                            let ( data,ext,tag) = i;
                            let  send_data = match asy.encrypt(data, ext) {
                                EncryptRes::EncryptSucc(d) => {
                                    d
                                }
                                _ => { data.to_vec()}
                            };
                            let real_pkg = self.parser.package_tf(send_data, ext,tag);
                            match sender.send_msg(real_pkg).await
                            {
                                Ok(_) => { }
                                Err(e) => {
                                    Stop!(self,runtime,plug_collect,e,on_ret);
                                }
                            }
                        }
                    }else {
                        match asy.encrypt(&v.0, v.1) {
                            EncryptRes::EncryptSucc(d) => {
                                v.0 = d;
                            }
                            EncryptRes::NotChange => {}
                            _ => {}
                        };
                        if let Err(e) = self.write_msg( v.0, v.1).await{
                            Stop!(self,runtime,plug_collect,e,on_ret);
                        }
                    }
                }else{
                    sleep(Duration::from_millis(1)).await;
                }
            }

            if !self.still_runing().await
            {
                break;
            }

        }
        self.stop().await;
        plug_collect.on_stop().await;
        self.clear_sender().await;
        println!("waiting for recv worker!");
        runtime.shutdown_background();
        println!("recv worker end!");
        on_ret.await;
        Ok(())
    }

    #[allow(unused_must_use)]
    pub async fn run_with_sender<CP,F>(
        &self,
        rx:Receiver<Vec<u8>>,
        asy_cry_ignore:Option<&Vec<u32>>,
        msg_split_ignore:Option<&Vec<u32>>,
        on_ret: F,
        sender: Arc<SE>
    ) -> Result<(),UdpClientErr>

        where
            F: futures::Future<Output=()>
    {

        // In a loop, read data from the socket and write the data back.
        let mut heartbeat_t = SystemTime::now();
        let mut asy = DefAsyCry::create();
        if let Some(v) = asy_cry_ignore{
            asy.extend_ignore(v.as_slice());
        }
        let mut spliter = DefMsgSplit::new();
        if let Some(v) = msg_split_ignore{
            spliter.extend_ignore(v.as_slice());
        }
        let mut package = None;

        let err:Arc<Mutex<Option<USErr>>> = Arc::new(Mutex::new(None));
        let sender_cp = sender.clone();
        let run_cp = self.runing.clone();
        let err_cp = err.clone();

        self.set_sender(sender.clone()).await;

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .build()
            .unwrap();

        let recv_worker = runtime.spawn(async move {
            let r = {
                let r = run_cp.lock().await;
                *r
            };
            'Out: while r {
                match rx.recv().await{
                    Ok(buf) => {
                        match sender_cp.check_recv(&buf[..]).await
                        {
                            Err(USErr::EmptyMsg)=>{}
                            Err(e)=>{
                                let mut err = err_cp.lock().await;
                                *err = Some(e.into());
                                let mut run = run_cp.lock().await;
                                *run = false;
                                break;
                            }
                            _=>{}
                        };
                        while sender_cp.need_check().await {
                            match sender_cp.check_recv(&[]).await{
                                Err(USErr::EmptyMsg)=>{}
                                Err(e)=>{
                                    let mut err = err_cp.lock().await;
                                    *err = Some(e.into());
                                    let mut run = run_cp.lock().await;
                                    *run = false;
                                    break 'Out;
                                }
                                _=>{}
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("recv from {:?}",e);
                        let mut run = run_cp.lock().await;
                        *run = false;
                        break;
                    }
                }
            }
        });


        if let Ok(pub_key_data) = asy.build_pub_key().await{
            if let Err(e) =  self.write_msg( pub_key_data, 10).await{
                StopNoPlug!(self,runtime,e,on_ret);
            }
        }

        let mut subpackager = DefSubpackage::new();


        loop {
            // read request
            //println!("read the request....");
            {
                let err = err.lock().await;
                if let Some(e) = (*err).clone(){
                    StopNoPlug!(self,runtime,e,on_ret);
                }
            };
            match sender.pop_recv_msg().await{
                Ok(v) => {
                    package = subpackager.subpackage(&v[..],v.len());
                }
                Err(USErr::EmptyMsg) => {}
                Err(e) => {
                    StopNoPlug!(self,runtime,e,on_ret);
                }
            }

            if package.is_none() && subpackager.need_check(){
                package = subpackager.subpackage(&[],0);
            }

            if let Some( mut d) = package {
                package = None;
                let mut _temp_data = None;
                let msg = self.parser.parse_tf(&mut d);
                //dbg!(&msg);
                if let Some(mut m) = msg {
                    //----------------------------------
                    let mut immediate_send = None;
                    let mut override_msg = None;
                    match asy.try_decrypt(m.msg, m.ext).await
                    {
                        EncryptRes::EncryptSucc(d) => {
                            override_msg = Some(d);
                        }
                        EncryptRes::RPubKey(d) => {
                            immediate_send = Some(d.0);
                            m.ext = d.1;
                        }
                        EncryptRes::ErrMsg(d) => {
                            immediate_send = Some(d.0);
                            m.ext = d.1;
                        }
                        EncryptRes::NotChange => {}
                        EncryptRes::Break => { continue; }
                    };
                    if let Some(v) = immediate_send
                    {
                        if let Err(e) = self.write_msg(v, m.ext).await{
                            StopNoPlug!(self,runtime,e,on_ret);
                        }
                        continue;
                    }
                    if let Some(ref v) = override_msg
                    {
                        m.msg = v.as_slice();
                    }
                    // if m.ext != 9
                    // {println!("{:?} {}",&m.msg,m.ext);}
                    if spliter.need_merge(&m)
                    {
                        if let Some((data,ext)) = spliter.merge(&m)
                        {
                            _temp_data = Some(data);
                            m.ext = ext;
                            m.msg = _temp_data.as_ref().unwrap().as_slice();
                        }else{
                            continue;
                        }
                    }
                    if let Some((d,e)) = self.handler.handle_ex(m).await
                    {
                        self.send(d,e).await;
                    }
                }
                package = None;
            }
            if let Err(e) = sender.check_send().await{
                StopNoPlug!(self,runtime,e,on_ret);
            }

            if let Ok(n) = SystemTime::now().duration_since(heartbeat_t)
            {
                if n >= self.heartbeat_dur
                {
                    heartbeat_t = SystemTime::now();
                    if let Err(e) = self.write_msg(vec![9], 9).await{
                        StopNoPlug!(self,runtime,e,on_ret);
                    }
                }
            }

            if asy.can_encrypt() {
                let mut _data = self.pop_msg().await;
                if let Some(mut v) = _data {
                    if spliter.need_split(v.0.len(),v.1)
                    {
                        let msgs = spliter.split(&mut v.0,v.1);
                        for i in msgs.into_iter(){
                            let ( data,ext,tag) = i;
                            let  send_data = match asy.encrypt(data, ext) {
                                EncryptRes::EncryptSucc(d) => {
                                    d
                                }
                                _ => { data.to_vec()}
                            };
                            let real_pkg = self.parser.package_tf(send_data, ext,tag);
                            match sender.send_msg(real_pkg).await
                            {
                                Ok(_) => { }
                                Err(e) => {
                                    StopNoPlug!(self,runtime,e,on_ret);
                                }
                            }
                        }
                    }else {
                        match asy.encrypt(&v.0, v.1) {
                            EncryptRes::EncryptSucc(d) => {
                                v.0 = d;
                            }
                            EncryptRes::NotChange => {}
                            _ => {}
                        };
                        if let Err(e) = self.write_msg(v.0, v.1).await{
                            StopNoPlug!(self,runtime,e,on_ret);
                        }
                    }
                }else{
                    sleep(Duration::from_millis(1)).await;
                }
            }

            if !self.still_runing().await
            {
                break;
            }

        }

        self.stop().await;
        self.clear_sender().await;
        println!("waiting for recv worker!");
        runtime.shutdown_background();
        println!("recv worker end!");
        on_ret.await;
        Ok(())
    }
}
