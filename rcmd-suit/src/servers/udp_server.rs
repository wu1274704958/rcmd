use std::{collections::HashMap, marker::PhantomData, sync::{Arc}};
use async_std::channel::Receiver;
use tokio::runtime::{self, Runtime};
use crate::{handler::{ Handle, SubHandle}, plug};
use crate::config_build::Config;
use crate::plug::{PlugMgr,Plug};
use crate::agreement::Agreement;
use tokio::net::{UdpSocket};
use std::ops::{Add, AddAssign};
use crate::ab_client::{ABClient,State};
use crate::subpackage::{DefSubpackage,Subpackage};
use crate::asy_cry::{DefAsyCry,AsyCry,EncryptRes,NoAsyCry};
use crate::utils::udp_sender::{DefUdpSender, UdpSender, USErr};
use std::net::SocketAddr;
use std::time::SystemTime;
use tokio::sync::{Mutex};
use crate::utils::msg_split::{DefMsgSplit, MsgSplit};
use crate::tools::platform_handle;
use ahash::RandomState;
use ahash::CallHasher;
use std::hash::BuildHasher;
use std::future::Future;

pub struct UdpServer<LID,ABC,P,SH,H,PL,PLM>
    where SH : SubHandle<ABClient=ABC,Id=LID>,
          H : Handle<SH>,
          PL : Plug,
          PLM: PlugMgr<PL>,
          ABC: ABClient<LID = LID>
{
    _a:PhantomData<SH>,
    _b:PhantomData<PL>,
    pub config:Arc<Config>,
    pub runtime:Runtime,
    pub logic_id:Arc<Mutex<LID>>,
    pub clients:Arc<Mutex<HashMap<LID,Box<ABC>>>>,
    pub parser:Arc<P>,
    pub handler:Arc<H>,
    pub plug_mgr:Arc<PLM>,
    pub dead_plug_mgr:Arc<PLM>,
    pub buf_len:usize,
    pub channel_buf:usize,
}

impl<LID,ABC,P,SH,H,PL,PLM> UdpServer<LID,ABC,P,SH,H,PL,PLM>
    where SH : SubHandle<ABClient=ABC,Id=LID>,
          H : Handle<SH> + Send + std::marker::Sync,
          PL : Plug<ABClient=ABC,Id=LID,Config=Config>,
          PLM: PlugMgr<PL> + Send + std::marker::Sync,
          P : Agreement + Send + std::marker::Sync,
          LID : AddAssign + Clone + Copy + Eq + std::hash::Hash+num_traits::identities::Zero + num_traits::identities::One + Send,
          ABC: ABClient<LID = LID> + Send
{
    pub fn new(
        handler:Arc<H>,
        parser:Arc<P>,
        plug_mgr:Arc<PLM>,
        dead_plug_mgr:Arc<PLM>,
        config:Config,
    )
        -> UdpServer<LID,ABC,P,SH,H,PL,PLM>
    {
        let runtime = runtime::Builder::new_multi_thread()
            .worker_threads(config.thread_count)
            .build()
            .unwrap();
        let logic_id = Arc::new(Mutex::new(LID::zero()));
        UdpServer::<LID,ABC,P,SH,H,PL,PLM>{
            _a: Default::default(),
            _b: Default::default(),
            handler,
            parser,
            plug_mgr,
            dead_plug_mgr,
            config:Arc::new(config),
            runtime,
            logic_id,
            clients: Arc::new(Mutex::new(HashMap::new())),
            buf_len: 65536,
            channel_buf : 10000,
        }
    }

    pub fn with_clients(
        handler:Arc<H>,
        parser:Arc<P>,
        plug_mgr:Arc<PLM>,
        dead_plug_mgr:Arc<PLM>,
        config:Config,
        clients: Arc<Mutex<HashMap<LID,Box<ABC>>>>,
    )
        -> UdpServer<LID,ABC,P,SH,H,PL,PLM>
    {
        let runtime = runtime::Builder::new_multi_thread()
            .worker_threads(config.thread_count)
            .build()
            .unwrap();
        let logic_id = Arc::new(Mutex::new(LID::zero()));
        UdpServer::<LID,ABC,P,SH,H,PL,PLM>{
            _a: Default::default(),
            _b: Default::default(),
            handler,
            parser,
            plug_mgr,
            dead_plug_mgr,
            config:Arc::new(config),
            runtime,
            logic_id,
            clients,
            buf_len: 65536,
            channel_buf : 10000,
        }
    }

    pub fn with_buf_len(
        handler:Arc<H>,
        parser:Arc<P>,
        plug_mgr:Arc<PLM>,
        dead_plug_mgr:Arc<PLM>,
        config:Config,
        buf_len:usize
    )
        -> UdpServer<LID,ABC,P,SH,H,PL,PLM>
    {
        let mut v = Self::new(handler,parser,plug_mgr,dead_plug_mgr,config);
        if buf_len > 0 { v.buf_len = buf_len; }
        v
    }


    async fn get_client_st(&self,id:LID)->Option<State>
    {
        let mut abs = self.clients.lock().await;
        if let Some(a) = abs.get(&id)
        {
            Some(a.state())
        }else{
            None
        }
    }

    async fn set_client_st(&self,id:LID,s:State)
    {
        let mut abs = self.clients.lock().await;
        if let Some( a) = abs.get_mut(&id)
        {
            if !a.state().is_wait_kill(){
                a.set_state(s)
            }
        }
    }

    async fn new_client(&self,local_addr: SocketAddr, addr: SocketAddr) -> LID
    {
        let mut logic_id = LID::zero();
        {
            let mut a = self.logic_id.lock().await;
            a.add_assign(LID::one());
            logic_id = *a;
        }
        let thread_id = std::thread::current().id();
        let a = ABC::create(local_addr,addr,logic_id,thread_id);
        {
            let mut abs = self.clients.lock().await;
            abs.insert(logic_id,Box::new(a));
        }
        logic_id
    }
    async fn del_client(&self,id:LID)->Option<Box<ABC>>
    {
        let mut res = None;
        let mut empty = false;
        {
            let mut abs = self.clients.lock().await;
            res = abs.remove(&id);
            empty = abs.is_empty();
        }
        if empty{
            let mut a = self.logic_id.lock().await;
            *a = LID::zero();
        }
        res
    }
}

pub async fn get_client_st_ex<LID,ABC>(clients:&Arc<Mutex<HashMap<LID,Box<ABC>>>>,id:LID)->Option<State>
    where
        LID : AddAssign + Clone + Copy + Eq + std::hash::Hash+num_traits::identities::Zero + num_traits::identities::One + Send,
        ABC: ABClient<LID = LID> + Send
{
    let abs = clients.lock().await;
    if let Some(a) = abs.get(&id)
    {
        Some(a.state())
    }else{
        None
    }
}

pub async fn set_client_st_ex<LID,ABC>(clients:&Arc<Mutex<HashMap<LID,Box<ABC>>>>,id:LID,s:State)
    where
        LID : AddAssign + Clone + Copy + Eq + std::hash::Hash+num_traits::identities::Zero + num_traits::identities::One + Send,
        ABC: ABClient<LID = LID> + Send
{
    let mut abs = clients.lock().await;
    if let Some(a) = abs.get_mut(&id)
    {
        if !a.state().is_wait_kill(){
            a.set_state(s)
        }
    }
}

async fn new_client_ex<LID,ABC>(clients:&Arc<Mutex<HashMap<LID,Box<ABC>>>>,logic_id_:&Arc<Mutex<LID>>,local_addr: SocketAddr, addr: SocketAddr) -> LID
    where
        LID : AddAssign + Clone + Copy + Eq + std::hash::Hash+num_traits::identities::Zero + num_traits::identities::One + Send,
        ABC: ABClient<LID = LID> + Send
{
    let mut logic_id = LID::zero();
    {
        let mut a = logic_id_.lock().await;
        a.add_assign(LID::one());
        logic_id = *a;
    }
    let thread_id = std::thread::current().id();
    let a = ABC::create(local_addr,addr,logic_id,thread_id);
    {
        let mut abs = clients.lock().await;
        abs.insert(logic_id,Box::new(a));
    }
    logic_id
}
async fn del_client_ex<LID,ABC>(clients:&Arc<Mutex<HashMap<LID,Box<ABC>>>>,logic_id:&Arc<Mutex<LID>>,id:LID)->Option<Box<ABC>>
    where
        LID : AddAssign + Clone + Copy + Eq + std::hash::Hash+num_traits::identities::Zero + num_traits::identities::One + Send,
        ABC: ABClient<LID = LID> + Send
{
    let mut res = None;
    let mut empty = false;
    {
        let mut abs = clients.lock().await;
        res = abs.remove(&id);
        empty = abs.is_empty();
    }
    if empty{
        let mut a = logic_id.lock().await;
        *a = LID::zero();
    }
    res
}


async fn clean<LID,ABC,PL,PLM,F>(
    dead_plugs_cp:Arc<PLM>,
    logic_id:LID,
    clients:Arc<Mutex<HashMap<LID, Box<ABC>>>>,
    conf:Arc<Config>,
    lid:Arc<Mutex<LID>>,
    on_ret:F,
)
    where PL : Plug<ABClient=ABC,Id=LID,Config=Config>,
          PLM: PlugMgr<PL> + Send + std::marker::Sync,
          LID : AddAssign + Clone + Copy + Eq + std::hash::Hash+num_traits::identities::Zero + num_traits::identities::One + Send,
          ABC: ABClient<LID = LID> + Send,
          F : futures::Future<Output=()>,
{
    dead_plugs_cp.run(logic_id.clone(),&clients,conf).await;
    del_client_ex(&clients,&lid,logic_id).await;
    on_ret.await;
}

#[allow(unused_must_use)]
#[allow(unused_variables)]
pub async fn run_in<LID,ABC,P,SH,H,PL,PLM,F,SE>
(
    clients:Arc<Mutex<HashMap<LID,Box<ABC>>>>,
    lid:Arc<Mutex<LID>>,
    conf:Arc<Config>,
    handler_cp:Arc<H>,
    parser_cp:Arc<P>,
    plugs_cp:Arc<PLM>,
    dead_plugs_cp:Arc<PLM>,
    socket:Arc<UdpSocket>,
    sender:Arc<SE>,
    addr:SocketAddr,
    on_ret:F,
    asy_cry_ignore:Option<&Vec<u32>>,
    msg_split_ignore:Option<&Vec<u32>>
)
    where SH : SubHandle<ABClient=ABC,Id=LID>,
          H : Handle<SH> + Send + std::marker::Sync,
          PL : Plug<ABClient=ABC,Id=LID,Config=Config>,
          PLM: PlugMgr<PL> + Send + std::marker::Sync,
          P : Agreement + Send + std::marker::Sync,
          LID : AddAssign + Clone + Copy + Eq + std::hash::Hash+num_traits::identities::Zero + num_traits::identities::One + Send,
          ABC: ABClient<LID = LID> + Send,
          F : futures::Future<Output=()>,
          SE : UdpSender + Send + std::marker::Sync + ?Sized
{
    let local_addr = socket.local_addr().unwrap();

    let logic_id = new_client_ex(&clients,&lid,local_addr,addr).await;

    let mut subpackager = DefSubpackage::new();
    let mut asy = DefAsyCry::create();
    if let Some(v) = asy_cry_ignore
    {
        asy.extend_ignore(v);
    }
    let mut spliter = DefMsgSplit::new();
    if let Some(v) = msg_split_ignore
    {
        spliter.extend_ignore(v);
    }
    let mut package = None;
    // In a loop, read data from the socket and write the data back.
    loop {

        {
            let st = get_client_st_ex(&clients,logic_id.clone()).await;
            if st.is_none() { println!(" begin ee1");return; }
            match st{
                Some(State::WaitKill) => {
                    clean(dead_plugs_cp.clone(),logic_id.clone(),clients.clone(),conf.clone(),lid.clone(),on_ret).await;
                    return;
                }
                _ => {}
            };
        }
        // read request
        // println!(" read the request....");
        match sender.pop_recv_msg().await{
            Ok(v) => {
                set_client_st_ex(&clients,logic_id.clone(), State::Busy).await;
                package = subpackager.subpackage(&v[..],v.len());
            }
            Err(USErr::EmptyMsg) => {}
            Err(e) => {
                eprintln!("recv error = {:?}",e);
                clean(dead_plugs_cp.clone(),logic_id.clone(),clients.clone(),conf.clone(),lid.clone(),on_ret).await;
                return;
            }
        }

        if package.is_none() && subpackager.need_check(){
            let b = SystemTime::now();
            package = subpackager.subpackage(&[],0);
            //println!("subpackage check use {} ms",SystemTime::now().duration_since(b).unwrap().as_millis());
        }

        if let Some(mut d) = package
        {
            package = None;
            let mut temp_data = None;
            //handle request
            let msg = {  parser_cp.parse_tf(&mut d) };
            //dbg!(&msg);
            if let Some(mut m) = msg {
                //----------------------------------
                let mut immediate_send = None;
                let mut override_msg = None;
                match asy.try_decrypt(m.msg,m.ext).await
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
                    EncryptRes::Break => {continue;}
                };
                if let Some(v) = immediate_send
                {
                    if let Err(_) = sender.send_msg(parser_cp.package_nor(v, m.ext)).await{
                        clean(dead_plugs_cp.clone(),logic_id.clone(),clients.clone(),conf.clone(),lid.clone(),on_ret).await;
                        return;
                    }
                    continue;
                }
                if let Some(ref v) = override_msg
                {
                    m.msg = v.as_slice();
                }
                let b = SystemTime::now();
                if spliter.need_merge(&m)
                {
                    if let Some((data,ext)) = spliter.merge(&m)
                    {
                        temp_data = Some(data);
                        m.ext = ext;
                        m.msg = temp_data.as_ref().unwrap().as_slice();
                    }else{
                        continue;
                    }
                }
                let respose = handler_cp.handle_ex(m, &clients, logic_id.clone()).await;
                if m.ext != 9 {println!("handle ext {} use {} ms",m.ext,SystemTime::now().duration_since(b).unwrap().as_millis());}
                if let Some((mut respose,mut ext)) = respose {
                    //---------------------------------
                    if spliter.need_split(respose.len(),ext)
                    {
                        let mut msgs = spliter.split(&mut respose,ext);
                        for i in msgs.into_iter(){
                            let (mut data,ext,tag) = i;
                            let mut send_data = match asy.encrypt(data, ext) {
                                EncryptRes::EncryptSucc(d) => {
                                    d
                                }
                                _ => { data.to_vec()}
                            };
                            if let Err(e) = sender.send_msg(parser_cp.package_tf(send_data, ext,tag)).await{
                                eprintln!("recv error = {:?}",e);
                                clean(dead_plugs_cp.clone(),logic_id.clone(),clients.clone(),conf.clone(),lid.clone(),on_ret).await;
                                return;
                            }
                        }
                    }else {
                        match asy.encrypt(&respose, ext) {
                            EncryptRes::EncryptSucc(d) => {
                                respose = d;
                                println!("send ext {}", ext);
                            }
                            _ => {}
                        };
                        if let Err(e) = sender.send_msg(parser_cp.package_nor(respose, ext)).await{
                            eprintln!("recv error = {:?}",e);
                            clean(dead_plugs_cp.clone(),logic_id.clone(),clients.clone(),conf.clone(),lid.clone(),on_ret).await;
                            return;
                        }
                    }
                }
            }
        }

        if let Err(e) = sender.check_send().await{
            eprintln!("recv error = {:?}",e);
            clean(dead_plugs_cp.clone(),logic_id.clone(),clients.clone(),conf.clone(),lid.clone(),on_ret).await;
            return;
        }

        //println!("{} handle the request....", logic_id);
        //println!("{} check the write_buf....", logic_id);
        let mut msg = None;
        {
            if let Some(mut cl) = clients.lock().await.get(&logic_id)
            {
                msg = cl.pop_msg();
            }
        }
        //------------------------------------------------
        if let Some((mut data,e)) = msg{
            if spliter.need_split(data.len(),e)
            {
                let mut msgs = spliter.split(&mut data,e);
                for i in msgs.into_iter(){
                    let (mut data,ext,tag) = i;
                    let mut send_data = match asy.encrypt(data, ext) {
                        EncryptRes::EncryptSucc(d) => {
                            d
                        }
                        _ => { data.to_vec()}
                    };
                    if let Err(_) = sender.send_msg(parser_cp.package_tf(send_data, ext, tag)).await{
                        clean(dead_plugs_cp.clone(),logic_id.clone(),clients.clone(),conf.clone(),lid.clone(),on_ret).await;
                        return;
                    };
                }
            }else {
                match asy.encrypt(&data, e) {
                    EncryptRes::EncryptSucc(d) => {
                        data = d;
                    }
                    _ => {}
                };
                if let Err(_) = sender.send_msg(parser_cp.package_nor(data, e)).await{
                    clean(dead_plugs_cp.clone(),logic_id.clone(),clients.clone(),conf.clone(),lid.clone(),on_ret).await;
                    return;
                };
            }
        }else {
            async_std::task::sleep(conf.min_sleep_dur).await;
        }
        set_client_st_ex(&clients,logic_id.clone(),State::Ready).await;
        plugs_cp.run_with_dur(logic_id.clone(),&clients,conf.clone()).await;
    }
}

pub async fn run_udp_server<LID,ABC,P,SH,H,PL,PLM>(
    server:UdpServer<LID,ABC,P,SH,H,PL,PLM>,
    msg_split_ignore:Option<&'static Vec<u32>>,
    asy_cry_ignore:Option<&'static Vec<u32>>
) -> Result<(), Box<dyn std::error::Error>>
    where SH : SubHandle<ABClient=ABC,Id=LID> + 'static,
          H : Handle<SH> + Send + std::marker::Sync + 'static,
          PL : Plug<ABClient=ABC,Id=LID,Config=Config> + 'static,
          PLM: PlugMgr<PL> + Send + std::marker::Sync + 'static,
          P : Agreement + Send + std::marker::Sync + 'static,
          LID : AddAssign + Clone + Copy + Eq + std::hash::Hash+num_traits::identities::Zero + num_traits::identities::One + Send + std::marker::Sync + 'static,
          ABC: ABClient<LID = LID> + Send + 'static

{
    let sock = Arc::new(UdpSocket::bind(server.config.addr).await?);
    platform_handle(sock.as_ref());

    let linker_map = Arc::new(Mutex::new(HashMap::<u64,Arc<DefUdpSender>>::new()));
    let hash_builder = RandomState::new();

    loop {
        let mut buf = Vec::with_capacity(server.buf_len);
        buf.resize(server.buf_len,0);
        match sock.recv_from(&mut buf[..]).await
        {
            Ok((len,addr)) => {

                let id = CallHasher::get_hash(&addr, hash_builder.build_hasher());
                let has = {
                    let mut map = linker_map.lock().await;
                    if let Some(link) = map.get(&id){
                        link.check_recv(&buf[0..len]).await;
                        while link.need_check().await { link.check_recv(&[]).await; }
                        true
                    }else { false }
                };
                if !has
                {
                    let mut sender = Arc::new(DefUdpSender::create(sock.clone(),addr));
                    {
                        sender.check_recv(&buf[0..len]).await;
                        println!("check_recv end -------------------------");
                        while sender.need_check().await { sender.check_recv(&[]).await; }
                        println!("-------------------------");
                        let mut map = linker_map.lock().await;
                        map.insert(id, sender.clone());
                    }
                    {
                        let linker_map_cp = linker_map.clone();
                        let clients = server.clients.clone();
                        let lid = server.logic_id.clone();
                        let conf = server.config.clone();
                        let handler_cp = server.handler.clone();
                        let parser_cp = server.parser.clone();
                        let plugs_cp = server.plug_mgr.clone();
                        let dead_plugs_cp:Arc<_> = server.dead_plug_mgr.clone();
                        let sock_cp = sock.clone();
                        dbg!(&addr);

                        server.runtime.spawn(run_in(
                            clients,lid,conf,handler_cp,parser_cp,plugs_cp,dead_plugs_cp,sock_cp,sender,
                            addr,async move{
                                let id_ = id;
                                let mut map = linker_map_cp.lock().await;
                                map.remove(&id_);
                                println!("disconnect id = {}",id_);
                            },asy_cry_ignore.clone(),msg_split_ignore.clone()
                        ));

                        println!("Spawn a client handler!!!");
                    }
                }
            }
            Err(e) => {
                eprintln!("err = {:?}",e);
            }
        }
    }

    Ok(())
}