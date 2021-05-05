
use std::{collections::VecDeque, io, net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs}, sync::{Arc,Mutex}, time::{Duration, SystemTime}};
use std::vec::Vec;
use tokio::{io::AsyncWriteExt, net::{UdpSocket}, time::sleep};

use crate::{agreement::{Agreement, Message}, asy_cry::{DefAsyCry,AsyCry,EncryptRes,NoAsyCry},  subpackage::{DefSubpackage,Subpackage}, utils::msg_split::{DefMsgSplit,MsgSplit}};
use crate::utils::udp_sender::{DefUdpSender,UdpSender,USErr};
use crate::tools::platform_handle;
use async_std::io::Error;
use async_std::future::Future;
use crate::client_handler::Handle;
use std::thread::Thread;

pub struct UdpClient<T,A>
    where T:Handle
{
    msg_queue:Arc<Mutex<VecDeque<(Vec<u8>,u32)>>>,
    runing:Arc<Mutex<bool>>,
    handler:Arc<T>,
    pub heartbeat_dur:Duration,
    pub nomsg_rest_dur:Duration,
    bind_addr:SocketAddr,
    pub buf_size:usize,
    parser:A
}

fn socket_addr_conv<T:ToSocketAddrs>(t:T) -> io::Result<SocketAddr>{
    t.to_socket_addrs()?.next().ok_or(io::Error::from(io::ErrorKind::InvalidInput))
}

impl <'a,T,A> UdpClient<T,A>
    where T:Handle,
          A : Agreement
{
    pub fn new(bind_addr:impl ToSocketAddrs,handler:Arc<T>,parser:A)-> Self
    {
        UdpClient::<T,A>{
            msg_queue : Arc::new(Mutex::new(VecDeque::new())),
            runing:Arc::new(Mutex::new(true)),
            handler,
            heartbeat_dur:Duration::from_secs(10),
            nomsg_rest_dur:Duration::from_millis(1),
            parser,
            bind_addr : socket_addr_conv(bind_addr).unwrap(),
            buf_size:1024 * 1024 * 10
        }
    }

    pub fn with_dur(bind_addr:impl ToSocketAddrs,handler:Arc<T>,parser:A,
                    heartbeat_dur:Duration,
                    nomsg_rest_dur:Duration)-> Self
    {
        UdpClient::<T,A>{
            msg_queue : Arc::new(Mutex::new(VecDeque::new())),
            runing:Arc::new(Mutex::new(true)),
            handler,
            heartbeat_dur,
            nomsg_rest_dur,
            parser,
            bind_addr:socket_addr_conv(bind_addr).unwrap(),
            buf_size:1024 * 1024 * 10
        }
    }

    pub fn with_msg_queue(bind_addr:impl ToSocketAddrs,handler:Arc<T>,parser:A,
                          msg_queue:Arc<Mutex<VecDeque<(Vec<u8>,u32)>>>)-> Self
    {
        UdpClient::<T,A>{
            msg_queue,
            runing:Arc::new(Mutex::new(true)),
            handler,
            heartbeat_dur:Duration::from_secs(10),
            nomsg_rest_dur:Duration::from_millis(1),
            parser,
            bind_addr: socket_addr_conv(bind_addr).unwrap(),
            buf_size:1024 * 1024 * 10
        }
    }

    pub fn with_msg_queue_runing(bind_addr:impl ToSocketAddrs,handler:Arc<T>,parser:A,
                                 msg_queue:Arc<Mutex<VecDeque<(Vec<u8>,u32)>>>,
                                 runing:Arc<Mutex<bool>>)-> Self
    {
        UdpClient::<T,A>{
            msg_queue,
            runing,
            handler,
            heartbeat_dur:Duration::from_secs(10),
            nomsg_rest_dur:Duration::from_millis(1),
            parser,
            bind_addr: socket_addr_conv(bind_addr).unwrap(),
            buf_size:1024 * 1024 * 10
        }
    }

    pub fn send(&self,data: Vec<u8>,ext:u32) {
        let mut a = self.msg_queue.lock().unwrap();
        {
            a.push_back((data,ext));
        }
    }

    pub fn stop(&self)
    {
        let mut b = self.runing.lock().unwrap();
        *b = false;
    }

    pub fn still_runing(&self)->bool
    {
        let b = self.runing.lock().unwrap();
        *b
    }

    fn pop_msg(&self)->Option<(Vec<u8>,u32)>
    {
        let mut queue = self.msg_queue.lock().unwrap();
        queue.pop_front()
    }


    async fn write_msg<SE>(&self, sender:&Arc<SE>, data: Vec<u8>, ext:u32)->Result<(),USErr>
    where SE : UdpSender + Send + std::marker::Sync
    {
        match sender.send_msg(self.parser.package_nor(data,ext)).await
        {
            Ok(_) => { Ok(()) }
            Err(e) => {
                self.stop();
                Err(e)
            }
        }
    }

    #[allow(unused_must_use)]
    pub async fn run<SE>(&self,ip:Ipv4Addr,port:u16,asy_cry_ignore:Option<&Vec<u32>>,
                     msg_split_ignore:Option<&Vec<u32>>) -> Result<(),USErr>

    where SE : UdpSender + Send + std::marker::Sync + 'static
    {
        let sock = Arc::new( UdpSocket::bind(self.bind_addr).await? );
        platform_handle(sock.as_ref());
        let addr = SocketAddr::new(IpAddr::V4(ip), port);
        sock.connect(addr).await?;

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

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .build()
            .unwrap();

       let recv_worker = runtime.spawn(async move {
            let r = {
                let r = run_cp.lock().unwrap();
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
                                    let mut err = err_cp.lock().unwrap();
                                    *err = Some(e.into());
                                    let mut run = run_cp.lock().unwrap();
                                    *run = false;
                                    break;
                                }
                                _=>{}
                            };
                            while sender_cp.need_check().await {
                                match sender_cp.check_recv(&[]).await{
                                    Err(USErr::EmptyMsg)=>{}
                                    Err(e)=>{
                                        let mut err = err_cp.lock().unwrap();
                                        *err = Some(e.into());
                                        let mut run = run_cp.lock().unwrap();
                                        *run = false;
                                        break 'Out;
                                    }
                                    _=>{}
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("recv from {:?}",e);
                        let mut err = err_cp.lock().unwrap();
                        *err = Some(e.into());
                        let mut run = run_cp.lock().unwrap();
                        *run = false;
                        break;
                    }
                }
            }
        });


        if let Ok(pub_key_data) = asy.build_pub_key().await{
            self.write_msg(&sender, pub_key_data, 10).await?;
        }

        let mut subpackager = DefSubpackage::new();

        loop {
            // read request
            //println!("read the request....");
            {
                let err = err.lock().unwrap();
                if let Some(e) = (*err).clone(){
                    self.stop();
                    recv_worker.await;
                    return Err(e);
                }
            };
            match sender.pop_recv_msg().await{
                Ok(v) => {
                    package = subpackager.subpackage(&v[..],v.len());
                }
                Err(USErr::EmptyMsg) => {}
                Err(e) => {
                    self.stop();
                    recv_worker.await;
                    return Err(e);
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
                        self.write_msg(&sender, v, m.ext).await?;
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
                    if let Some((d,e)) = self.handler.handle_ex(m)
                    {
                        self.send(d,e);
                    }
                }
                package = None;
            }else{
                sender.check_send().await?;
            }

            if let Ok(n) = SystemTime::now().duration_since(heartbeat_t)
            {
                if n > self.heartbeat_dur
                {
                    heartbeat_t = SystemTime::now();
                    self.write_msg(& sender, vec![9], 9).await?;
                }
            }

            if asy.can_encrypt() {
                let mut _data = self.pop_msg();
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
                                    self.stop();
                                    recv_worker.await;
                                    return Err(e);
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
                        self.write_msg(&sender, v.0, v.1).await?;
                    }
                }else{
                    sleep(Duration::from_millis(1)).await;
                }
            }

            if !self.still_runing()
            {
                break;
            }

        }
        sender.close_session().await;
        self.stop();
        println!("waiting for recv worker!");
        runtime.shutdown_background();
        println!("recv worker end!");
        Ok(())
    }

}