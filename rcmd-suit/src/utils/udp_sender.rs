use async_trait::async_trait;
use tokio::net::UdpSocket;
use std::sync::{Arc};
use num_traits::{One, Zero};
use std::collections::{VecDeque, HashMap};
use std::mem::size_of;
use async_std::net::SocketAddr;
use crate::subpackage::{UdpSubpackage,Subpackage};
use tokio::time::Duration;
use std::time::SystemTime;
use async_std::io::Error;
use crate::tools::{TOKEN_NORMAL};
use crate::utils::msg_split::{UdpMsgSplit, DefUdpMsgSplit, MsgSlicesInfo};
use tokio::sync::{Mutex,MutexGuard};
use crate::utils::udp_sender::USErr::Warp;
use std::fmt::Debug;
use crate::utils::udp_sender::SessionState::Has;
use crate::tools;
use futures::task::LocalSpawn;
use crate::utils::udp_sender::SpecialExt::send_recv;
use num_enum::TryFromPrimitive;
use std::convert::TryFrom;
use std::thread::sleep;
use crate::utils::data_current_limiter::DataCurrentLimiter;

#[async_trait]
pub trait UdpSender : std::marker::Send + std::marker::Sync{
    async fn send_msg(&self,v:Vec<u8>)->Result<(),USErr>;
    async fn check_recv(&self, data: &[u8]) -> Result<(),USErr>;
    async fn pop_recv_msg(&self) -> Result<Vec<u8>,USErr>;
    async fn need_check(&self)->bool;
    fn create(sock:Arc<UdpSocket>,addr:SocketAddr) ->Self where Self: Sized;
    async fn set_max_msg_len(&mut self,len:u16);
    fn max_msg_len(&self)->u16;
    async fn set_min_msg_len(&mut self,len:u16);
    fn min_msg_len(&self)->u16;
    fn max_cache_size(&self)->u16;
    fn set_max_cache_size(&mut self,s:u16);
    fn min_cache_size(&self)->u16;
    fn set_min_cache_size(&mut self,s:u16);
    fn set_time_out(&mut self,dur:Duration);
    async fn check_send(&self)->Result<(),USErr>;
    fn set_retry_times(&mut self,v:u16);
    fn retry_times(&self)->u16;
    async fn has_session(&self)->bool;
    async fn build_session(&self)->Result<(),USErr>;
    async fn close_session(&self)->Result<(),USErr>;
}

pub enum SessionState{
    Closed,
    Null,
    WaitResponse(u128,SystemTime,u16),
    WaitResponseCp(u128,SystemTime,u16),
    Has(u128)
}

impl SessionState{
    pub fn is_has(&self) -> bool
    {
        match self {
            SessionState::Null => {false}
            SessionState::WaitResponse(_, _, _) => {false}
            SessionState::WaitResponseCp(_, _, _) => {true}
            SessionState::Has(_) => {true}
            SessionState::Closed => {false}
        }
    }

    pub fn is_close(&self) -> bool
    {
        match self {
            SessionState::Closed => {true},
            _ => {false}
        }
    }
}

pub struct DefUdpSender{
    sock: Arc<UdpSocket>,
    max_len: u16,
    min_len: u16,
    cache_size:Arc<Mutex<u16>>,
    min_cache_size:u16,
    max_cache_size:u16,
    mid: Arc<Mutex<usize>>,
    queue: Arc<Mutex<VecDeque<(usize,Option<u32>)>>>,
    msg_map: Arc<Mutex<HashMap<usize,(Vec<u8>,SystemTime,u16)>>>,
    recv_cache: Arc<Mutex<HashMap<usize,(Vec<u8>,u32,u8,Option<Vec<u8>>)>>>,
    expect_id: Arc<Mutex<usize>>,
    addr:SocketAddr,
    subpacker: Arc<Mutex<UdpSubpackage>>,
    timeout: Duration,
    msg_split: Arc<Mutex<DefUdpMsgSplit>>,
    max_retry_times: u16,
    recv_queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
    error: Arc<Mutex<Option<USErr>>>,
    sid: Arc<Mutex<SessionState>>,
    adjust_cache_size_time:Arc<Mutex<SystemTime>>,
    avg_retry_times: Arc<Mutex<(u32,f32)>>,
    //data_current_limiter:Arc<DataCurrentLimiter>
}

#[derive(Clone)]
pub enum USErr{
    EmptyMsg,
    MsgCacheOverflow,
    RetryTimesLimit,
    Warp((i32,String)),
    ResponseSendSuccMiss,
    SendSuccMissCache,
    BadSessionID,
    NoSession,
    AlreadyHasSession,
    WaitSessionResponse,
    WaitSessionResponseCp,
    NotWaitingSessionResponse,
    NotWaitingSessionResponseCp,
    SendSessionFailed,
    AlreadyClosed,
    CpAlreadyClosed,
    CpRequestClosed
}

impl USErr {
    pub fn code(&self)->i32
    {
        match self {
            USErr::EmptyMsg => { 0 }
            USErr::MsgCacheOverflow => {-1}
            USErr::RetryTimesLimit => {-2}
            USErr::Warp(e) => { (*e).0 }
            USErr::ResponseSendSuccMiss => {-4},
            USErr::SendSuccMissCache=>{-5}
            USErr::BadSessionID=>{-6}
            USErr::NoSession=>{-7}
            USErr::AlreadyHasSession=>{-8}
            USErr::WaitSessionResponse=>{-9}
            USErr::SendSessionFailed=>{-10}
            USErr::NotWaitingSessionResponse=>{-11}
            USErr::NotWaitingSessionResponseCp=>{-12}
            USErr::WaitSessionResponseCp=>{-13},
            USErr::AlreadyClosed => {-14}
            USErr::CpAlreadyClosed => {-15}
            USErr::CpRequestClosed => {-16}
        }
    }

    pub fn err_str(&self)->String
    {
        match self {
            USErr::EmptyMsg => {"Empty Message".to_string()}
            USErr::MsgCacheOverflow => {"Message cache overflow".to_string()}
            USErr::RetryTimesLimit => {"Retry times reach the maximum".to_string()}
            USErr::Warp(e) => {  (*e).1.clone() }
            USErr::ResponseSendSuccMiss => { "Response send success miss message id".to_string() },
            USErr::SendSuccMissCache=>{"Send success miss the cache".to_string()}
            USErr::BadSessionID=>{"Bad session ID".to_string()}
            USErr::NoSession=>{"No Session".to_string()}
            USErr::AlreadyHasSession=>{"Already has session".to_string()}
            USErr::WaitSessionResponse=>{"Waiting session response".to_string()}
            USErr::SendSessionFailed=>{"Send session failed".to_string()}
            USErr::NotWaitingSessionResponse=>{"Not waiting session response".to_string()}
            USErr::NotWaitingSessionResponseCp=>{"Not waiting session response cp".to_string()}
            USErr::WaitSessionResponseCp=>{"Waiting session response cp".to_string()}
            USErr::AlreadyClosed=>{"Session already closed!".to_string()}
            USErr::CpAlreadyClosed => {"Cp Session already closed!".to_string()}
            USErr::CpRequestClosed => {"Cp Session request closed!".to_string()}
        }
    }
}

impl From<std::io::Error> for USErr
{
    fn from(e: Error) -> Self {
        let code = match e.raw_os_error() {
            None => {-3}
            Some(v) => {v}
        };
        let str = format!("{}",e);
        Warp((code,str))
    }
}

impl Debug for USErr
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("USErr")
            .field(&self.code())
            .field(&self.err_str())
            .finish()
    }
}

#[repr(u32)]
#[derive(TryFromPrimitive)]
#[derive(Copy, Clone)]
#[allow(non_camel_case_types)]
pub enum SpecialExt {
    send_recv = u32::max_value(),
    miss_cache = u32::max_value() - 1,
    send_sid = 100,
    response_send_sid = 101,
    response_cp_send_sid = 102,
    send_close = 103,
    skip_message = 104,
    err_already_has_sid = 5000,
    err_waiting_response = 5001,
    err_bad_session = 5002,
    err_no_session = 5003,
    err_not_wait_sid_resp = 5004,
    err_waiting_response_cp = 5005,
    err_not_wait_sid_resp_cp = 5006,
    err_already_closed = 5007,
}

impl Into<u32> for SpecialExt{
    fn into(self) -> u32
    {
        self as u32
    }
}

impl DefUdpSender
{
    async fn get_mid(&self)->usize
    {
        let mut mid = self.mid.lock().await;
        if *mid == usize::max_value()
        {
            *mid = usize::one();
        }else{
            *mid += usize::one();
        }
        *mid
    }
    async fn check_err(&self)->Result<(),USErr>
    {
        let err = self.error.lock().await;
        if let Some(e) = err.as_ref(){
            Err(e.clone())
        }else{
            Ok(())
        }
    }
    ///rid RecoveryID
    async fn warp(&self,v:&[u8],ext:u32,tag:u8,sub_head:Option<&[u8]>,rid:Option<u32>)->Result<Vec<u8>,USErr>
    {
        let sid = {
            if let Some(v) = self.get_sid().await{
                v
            }else{
                return Err(USErr::NoSession);
            }
        };
        let mid = self.get_mid().await;
        //println!("send id {}",mid);
        let res = Self::warp_ex(v,ext,tag,mid,sid,sub_head);
        self.push_cache(mid,res.clone(), rid).await?;
        Ok(res)
    }

    fn warp_ex(v:&[u8],ext:u32,tag:u8,mid:usize,sid:u128,sub_head:Option<&[u8]>)->Vec<u8>
    {
        //assert!(!v.is_empty());
        let len = v.len() + Self::package_len() + if let Some(sub) = sub_head{ sub.len() }else { 0 };
        let mut res = Vec::with_capacity(len);
        res.push(Self::magic_num_0());
        let len_buf = (len as u32).to_be_bytes();
        res.extend_from_slice(&len_buf[..]);
        res.push(Self::magic_num_1());
        let id_buf = mid.to_be_bytes();
        res.extend_from_slice(&id_buf[..]);
        res.push(Self::magic_num_2());
        let ext_buf = ext.to_be_bytes();
        res.extend_from_slice(&ext_buf[..]);
        res.push(tag);
        let sid_buf = sid.to_be_bytes();
        res.extend_from_slice(&sid_buf[..]);
        // push sub head
        if let Some(sub_head_buf) = sub_head{
            res.push(sub_head_buf.len() as u8);
            res.extend_from_slice(&sub_head_buf[..]);
        }else{
            res.push(0);
        }
        res.push(Self::magic_num_3());
        res.extend_from_slice(&v[..]);
        res.push(Self::magic_num_4());
        res
    }

    async fn push_cache(&self, id:usize, v:Vec<u8>,rid:Option<u32>) -> Result<(), USErr>
    {
        let mut queue = self.queue.lock().await;
        if queue.len() >= self.get_cache_size().await as usize  {
            return Err(USErr::MsgCacheOverflow);
            //self.drop_one_cache();
        }
        queue.push_back((id,rid));
        let mut msg_map = self.msg_map.lock().await;
        msg_map.insert(id,(v,SystemTime::now(),1));
        Ok(())
    }

    async fn remove_cache(&self,mid:usize)-> Option<(Vec<u8>,SystemTime,u16)>
    {
        let mut queue = self.queue.lock().await;
        let mut rmi = None;
        for (i,v) in queue.iter().enumerate(){
            if (*v).0 == mid
            {
                rmi = Some(i);
                break;
            }
        }
        match rmi {
            None => {
                //eprintln!("remove_cache not found mid {}",mid);
            }
            Some(v) => {
                queue.remove(v);
            }
        }
        let mut msg_map = self.msg_map.lock().await;
        msg_map.remove(&mid)
    }

    async fn unwarp(&self,data:&[u8])-> Result<Vec<u8>,USErr>
    {
        //print!("unwarp 1 {} ",data.len());
        let mut d = None;
        if data.is_empty() && self.need_check_in().await
        {
            let mut recv_cache = self.recv_cache.lock().await;
            match recv_cache.remove(&self.get_expect().await){
                None => {}
                Some(v) => {
                    self.next_expect().await;
                    if v.0.is_empty(){ d = None; }
                    else{ d = Some(v);}
                }
            }
        }
        if d.is_none() {
            let package_v = {
                let mut subpacker = self.subpacker.lock().await;
                subpacker.subpackage(data, data.len())
            };
            match package_v
            {
                None => { Err(USErr::EmptyMsg) }
                Some(v) => {
                    //print!(" step2 ");
                    let (msg, id, ext,tag,sid,sub_head) = self.unwarp_ex(v.as_slice());
                    //print!(" step3 ext {} id {} \n",ext,id);
                    if self.check_send_recv(msg,ext,tag,id,sid).await?
                    {
                        //print!("\n");
                        return Err(USErr::EmptyMsg);
                    }
                    //print!(" step4  ");
                    if self.is_close().await{
                        self.send_ext(SpecialExt::err_already_closed.into()).await?;
                        return Err(USErr::EmptyMsg);
                    }
                    //print!(" step5  ");
                    if let Some(sid_) = self.get_sid().await{
                        if sid_ != sid
                        {
                            self.send_ext(SpecialExt::err_bad_session.into()).await?;
                            return Err(USErr::EmptyMsg);
                        }
                    }else{
                        self.send_ext(SpecialExt::err_no_session.into()).await?;
                        return Err(USErr::EmptyMsg);
                    }
                    //print!(" step6 id {} \n",id);
                    self.send_recv(id).await?;
                    //println!("recv msg {}",id);
                    let mut except = self.expect_id.lock().await;
                    if id == *except {
                        Self::next_expect_ex(&mut except);
                        let mut msg_split = self.msg_split.lock().await;
                        if msg_split.need_merge(tag){
                            if let Some(v) = msg_split.merge(msg,ext,tag,sub_head)
                            {
                                return Ok(v);
                            }else { Err(USErr::EmptyMsg) }
                        }else{
                            return Ok(msg.to_vec());
                        }
                    }else if id > *except
                    {
                        let mut recv_cache = self.recv_cache.lock().await;
                        recv_cache.insert(id, (msg.to_vec(), ext,tag,if sub_head.len() == 0 { None }else { Some(sub_head.to_vec()) } ));
                        Err(USErr::EmptyMsg)
                    } else {
                        Err(USErr::EmptyMsg)
                    }
                }
            }
        }else {
            if let Some((msg, ext, tag,sub_head)) = d {
                let mut msg_split = self.msg_split.lock().await;
                if msg_split.need_merge(tag) {
                    if let Some(v) = msg_split.merge(msg.as_slice(), ext, tag,sub_head.unwrap().as_slice())
                    {
                        return Ok(v);
                    }
                } else {
                    return Ok(msg);
                }
            }
            Err(USErr::EmptyMsg)
        }
    }

    async fn check_send_recv(&self,msg:&[u8],ext:u32,tag:u8,id:usize,sid:u128) -> Result<bool,USErr>
    {
        if msg.len() == 1 && msg[0] == 199 && tag == TOKEN_NORMAL {
            //println!("get inside msg type ext = {}",ext);
            let sp_ext = SpecialExt::try_from(ext).unwrap();
            match sp_ext {
                SpecialExt::send_recv => {
                    //println!("op recv msg {}",id);
                    self.remove_cache(id).await;
                    // if let Some((_,t,times)) = self.remove_cache(id).await
                    // {
                    //     //println!("op recv msg id = {} times = {}",id,times);
                    //     self.check_msg_cache_queue().await;
                    // }
                    Ok(true)
                }
                SpecialExt::skip_message => {
                    println!("op send skip message id = {}",id);
                    self.send_recv(id).await?;
                    let mut expect_id = self.expect_id.lock().await;
                    if *expect_id == id{
                        Self::next_expect_ex(&mut expect_id);
                    }else if id > *expect_id{
                        let mut recv_cache = self.recv_cache.lock().await;
                        recv_cache.insert(id, (Vec::new(), ext,tag,None));
                    }
                    Ok(true)
                }
                SpecialExt::send_sid => {
                    let mut sid_ = self.sid.lock().await;
                    match *sid_ {
                        SessionState::Null => {
                            *sid_ = SessionState::WaitResponseCp(sid, SystemTime::now(), 1);
                            drop(sid);
                            self.send_sid_response(sid, SpecialExt::response_send_sid).await?;
                        }
                        SessionState::WaitResponse(_, _, _) => {
                            self.send_ext(SpecialExt::err_waiting_response).await?;
                        }
                        SessionState::WaitResponseCp(_, _, _) => {}
                        SessionState::Has(v) => {}
                        SessionState::Closed => {
                            self.send_ext(SpecialExt::err_already_closed).await?;
                        }
                    }
                    Ok(true)
                }
                SpecialExt::response_send_sid => {
                    let mut sid_ = self.sid.lock().await;
                    match *sid_ {
                        SessionState::Null => {
                            self.send_ext(SpecialExt::err_not_wait_sid_resp).await?;
                        }
                        SessionState::WaitResponse(v, _, _) => {
                            if v != sid {
                                self.send_ext(SpecialExt::err_bad_session).await?;
                            } else {
                                *sid_ = SessionState::Has(sid);
                                drop(sid_);
                                self.send_sid_response(sid, SpecialExt::response_cp_send_sid).await?;
                            }
                        }
                        SessionState::WaitResponseCp(v, _, _) => {
                            self.send_ext(SpecialExt::err_waiting_response_cp).await?;
                        }
                        SessionState::Has(v) => {
                            if v != sid {
                                self.send_ext(SpecialExt::err_already_has_sid).await?;
                            }
                        }
                        SessionState::Closed => {
                            self.send_ext(SpecialExt::err_already_closed).await?;
                        }
                    }
                    Ok(true)
                }
                SpecialExt::response_cp_send_sid => {
                    let mut sid_ = self.sid.lock().await;
                    match *sid_ {
                        SessionState::Null => {
                            self.send_ext(SpecialExt::err_not_wait_sid_resp_cp).await?;
                        }
                        SessionState::WaitResponse(v, _, _) => {
                            self.send_ext(SpecialExt::err_waiting_response).await?;
                        }
                        SessionState::WaitResponseCp(v, _, _) => {
                            if v != sid {
                                self.send_ext(SpecialExt::err_bad_session).await?;
                            } else {
                                *sid_ = SessionState::Has(sid);
                                drop(sid_);
                            }
                        }
                        SessionState::Has(v) => {
                            if v != sid {
                                self.send_ext(SpecialExt::err_already_has_sid).await?;
                            }
                        }
                        SessionState::Closed => {
                            self.send_ext(SpecialExt::err_already_closed).await?;
                        }
                    }
                    Ok(true)
                }
                SpecialExt::err_already_has_sid => {
                    Err(USErr::AlreadyHasSession)
                }
                SpecialExt::err_waiting_response => {
                    Err(USErr::WaitSessionResponse)
                }
                SpecialExt::err_waiting_response_cp => {
                    Err(USErr::WaitSessionResponseCp)
                }
                SpecialExt::err_bad_session => {
                    Err(USErr::BadSessionID)
                }
                SpecialExt::err_not_wait_sid_resp => {
                    Err(USErr::NotWaitingSessionResponse)
                }
                SpecialExt::err_no_session => {
                    Err(USErr::NoSession)
                }
                SpecialExt::err_not_wait_sid_resp_cp => {
                    Err(USErr::NotWaitingSessionResponseCp)
                }
                SpecialExt::err_already_closed => {
                    Err(USErr::CpAlreadyClosed)
                }
                SpecialExt::send_close => {
                    let mut sid_ = self.sid.lock().await;
                    match *sid_ {
                        SessionState::Has(v) => {
                            if sid != v {
                                self.send_ext(SpecialExt::err_bad_session).await?;
                            }else {
                                println!("cp request close!");
                                *sid_ = SessionState::Closed;
                                return Err(USErr::CpRequestClosed);
                            }
                        }
                        _ => {}
                    }
                    Ok(true)
                }
                _ => { Ok(false) }

            }
        }else{
            Ok(false)
        }
    }

    async fn send_ext(&self,ext:SpecialExt) -> Result<usize,USErr>
    {
        eprintln!("send ext {}",ext as u32);
        let v = Self::warp_ex(&[199],ext.into(),TOKEN_NORMAL,0,0,None);
        self.send(v.as_slice()).await
    }

    async fn send_sid_response(&self,sid:u128,ext:SpecialExt) -> Result<usize,USErr>
    {
        let v = Self::warp_ex(&[199],ext.into(),TOKEN_NORMAL,0,sid,None);
        self.send(v.as_slice()).await
    }

    async fn next_expect(&self)->usize
    {
        let mut expect_id = self.expect_id.lock().await;
        if *expect_id == usize::max_value()
        {
            *expect_id = usize::one();
        }else{
            *expect_id += usize::one();
        }
        *expect_id
    }

    fn next_expect_ex(expect_id:&mut MutexGuard<usize>)->usize
    {
        if **expect_id == usize::max_value()
        {
            **expect_id = usize::one();
        }else{
            **expect_id += usize::one();
        }
        **expect_id
    }

    async fn get_expect(&self)->usize
    {
        let mut expect_id = self.expect_id.lock().await;
        *expect_id
    }

    async fn send_recv(&self,id:usize) -> Result<usize,USErr>
    {
        let sid = {
            if let Some(v) = self.get_sid().await{
                v
            }else{
                return Err(USErr::NoSession);
            }
        };
        let v = Self::warp_ex(&[199],SpecialExt::send_recv.into(),TOKEN_NORMAL,id,sid,None);
        self.send(v.as_slice()).await?;
        self.send(v.as_slice()).await
    }

    async fn send(&self,d:&[u8]) -> Result<usize,USErr>
    {
        let sock = self.sock.clone();
        let addr = self.addr;
        Self::send_ex(sock,addr,d).await
    }

    async fn send_ex(sock:Arc<UdpSocket>,addr:SocketAddr,d:&[u8]) -> Result<usize,USErr>
    {
        //println!("send_ex ..........................");
        match sock.send_to(d,addr).await{
            Ok(l) => {
                Ok(l)
            }
            Err(e) => {
                eprintln!("udp send msg failed len = {} {:?}",d.len(),e);
                Err(e.into())
            }
        }
        //if len != d.len(){  eprintln!("udp send msg failed expect len {} get {}",d.len(),len); }
    }

    fn unwarp_ex<'a>(&self,data: &'a [u8])->(&'a[u8],usize,u32,u8,u128,&'a[u8])
    {
        const u8l:usize =  size_of::<u8>();
        const u32l:usize =  size_of::<u32>();
        const usizel:usize =  size_of::<usize>();
        const u128l:usize =  size_of::<u128>();

        let id_p = u8l + u32l;
        let ext_p = id_p + usizel + u8l;
        let tag = data[ext_p + u32l];
        let sid_p = ext_p + u32l + u8l;
        let sub_head_len = data[sid_p + u128l];
        let sub_head_p = sid_p + u128l + u8l;
        let msg_p = sub_head_p + sub_head_len as usize + u8l;

        let mut id_buf = [0u8;usizel];
        id_buf.copy_from_slice(&data[id_p..(id_p + usizel)]);
        let mut ext_buf = [0u8;u32l];
        ext_buf.copy_from_slice(&data[ext_p..(ext_p + u32l)]);
        let mut sid_buf = [0u8;size_of::<u128>()];
        sid_buf.copy_from_slice(&data[sid_p..(sid_p + size_of::<u128>())]);

        (
            &data[msg_p..data.len()],
            usize::from_be_bytes(id_buf),
            u32::from_be_bytes(ext_buf),
            tag,
            u128::from_be_bytes(sid_buf),
            &data[sub_head_p..(sub_head_p + sub_head_len as usize)]
        )
    }

    async fn need_check_in(&self)-> bool
    {
        let recv_cache = self.recv_cache.lock().await;
        !recv_cache.is_empty() && recv_cache.contains_key(&self.get_expect().await)
    }

    async fn need_check(&self) -> bool {
        let subpacker = self.subpacker.lock().await;
        subpacker.need_check() || self.need_check_in().await
    }

    const fn magic_num_0()->u8 {3}
    const fn magic_num_1()->u8 {1}
    const fn magic_num_2()->u8 {7}
    const fn magic_num_3()->u8 {2}
    const fn magic_num_4()->u8 {6}


    const fn package_len()->usize
    {
        size_of::<u8>() * 5 + size_of::<usize>() + size_of::<u32>() * 2 + size_of::<u128>()
    }

    async fn adjust_unit_size(&self,times_frequency:f32)
    {
        let mut msg_split = self.msg_split.lock().await;
        let mut cache_size = self.cache_size.lock().await;
        let curr_cache_size = *cache_size as usize;
        let mut msg_map = self.msg_map.lock().await;
        let cache_len = msg_map.len();
        if msg_split.is_max_unit_size(){
            if cache_len >= curr_cache_size / 2  && times_frequency > 2f32 {
                if curr_cache_size as u16 <= self.min_cache_size
                {
                    msg_split.down_unit_size();
                    self.try_recovery_msg(&mut msg_map,&mut msg_split).await;
                    println!("down unit size curr = {} ",msg_split.unit_size());
                }else{
                    *cache_size = self.min_cache_size;
                    msg_split.down_unit_size();
                    self.try_recovery_msg(&mut msg_map,&mut msg_split).await;
                    println!("down unit size curr = {} ",msg_split.unit_size());
                }
            }else if cache_len == curr_cache_size && times_frequency <= 1f32 {
                let v = self.adjust_cache_size_ex(&mut cache_size,1);
                println!("up cache size curr = {} ",v);
            }
        }else{
            if cache_len >= curr_cache_size / 2  && times_frequency > 2f32 {
                *cache_size = self.min_cache_size;
                msg_split.down_unit_size();
                self.try_recovery_msg(&mut msg_map,&mut msg_split).await;
                println!("down unit size curr = {} ",msg_split.unit_size());
            }else if cache_len == curr_cache_size && times_frequency <= 1f32 {
                msg_split.up_unit_size();
                println!("up unit size curr = {} ",msg_split.unit_size());
            }
        }

    }

    async fn try_recovery_msg(&self,msg_map:&mut MutexGuard<'_,HashMap<usize,(Vec<u8>,SystemTime,u16)>>,msg_split:&mut MutexGuard<'_,DefUdpMsgSplit>)
    {
        println!("try_recovery_msg  b");
        let mut queue = self.queue.lock().await;
        if !queue.is_empty() {
            let mut i = (queue.len()) as isize;
            loop {
                i -= 1;
                if i < 0 { break; }
                if let Some(id) = queue.get_mut(i as usize)
                {
                    if let Some(ref sub_id) = (*id).1 {
                        if msg_split.recovery(*sub_id) {
                            print!("Recovery id {} mid {} ", *sub_id, (*id).0);
                            if let Some(msg) = msg_map.get_mut(&(*id).0)
                            {
                                print!("Recovery step2 \n");
                                (*msg).2 = 1;
                                (*msg).0 = Self::warp_ex(
                                    &[199], SpecialExt::skip_message.into(),
                                    TOKEN_NORMAL, (*id).0, 0, None);

                                (*id).1 = None;
                            }
                        }
                    }
                }
            }
        }
        println!("try_recovery_msg  e ----------------");
    }

    async fn get_cache_len(&self) ->usize
    {
        let msg_map = self.msg_map.lock().await;
        msg_map.len()
    }

    async fn need_split(&self,len:usize)->bool
    {
        let mut msg_split = self.msg_split.lock().await;
        msg_split.need_split(len)
    }

    async fn set_error(&self,e:USErr)
    {
        let mut err = self.error.lock().await;
        *err = Some(e);
    }

    async fn set_sid(&self,v:u128)
    {
        let mut sid = self.sid.lock().await;
        *sid = SessionState::Has(v)
    }

    async fn get_sid(&self) -> Option<u128>
    {
        let sid = self.sid.lock().await;
        match *sid {
            SessionState::Null => {None}
            SessionState::WaitResponse(_, _, _) => {None}
            SessionState::WaitResponseCp(v,_,_) => {Some(v)}
            SessionState::Has(v) => {Some(v)}
            SessionState::Closed => {None}
        }
    }

    async fn has_sid(&self) -> bool
    {
        let sid = self.sid.lock().await;
        sid.is_has()
    }

    async fn is_close(&self) -> bool
    {
        let sid = self.sid.lock().await;
        sid.is_close()
    }

    async fn eq_sid(&self,v:u128) -> bool
    {
        let sid = self.sid.lock().await;
        match *sid {
            SessionState::Null => {false}
            SessionState::WaitResponse(_, _, _) => {false}
            SessionState::WaitResponseCp(v_,_,_) => {v == v_}
            SessionState::Has(v_) => {v == v_}
            SessionState::Closed => {false}
        }
    }

    async fn is_waiting_session(&self) -> bool
    {
        let sid = self.sid.lock().await;
        if let SessionState::WaitResponse(_,_,_) = *sid{
            true
        }else{
            false
        }
    }

    async fn send_session(&self) -> Result<(),USErr>
    {
        let sid_ = tools::uuid();
        let v = Self::warp_ex(&[199],SpecialExt::send_sid.into(),TOKEN_NORMAL,0,sid_,None);
        {
            let mut sid = self.sid.lock().await;
            *sid = SessionState::WaitResponse(sid_, SystemTime::now(), 1);
        }
        self.send(v.as_slice()).await?;
        Ok(())
    }

    async fn get_cache_size(&self) -> u16
    {
        let v = self.cache_size.lock().await;
        *v
    }

    async fn adjust_cache_size(&self,f:i16,r:i16) ->u16
    {
        let mut cs = self.cache_size.lock().await;
        let mut v = *cs as i16 / 10;
        if v <= 0 { v = 1; }
        let mut new = *cs as i16 + v * r * f;
        if new < self.min_cache_size as i16 { new = self.min_cache_size as i16; }
        if new > self.max_cache_size as i16 { new = self.max_cache_size as i16; }
        *cs = new as u16;
        new as u16
    }

    fn adjust_cache_size_no_await(&self,cs:&mut MutexGuard<u16>,f:i16,r:i16) ->u16
    {
        let mut v = **cs as i16 / 10;
        if v <= 0 { v = 1; }
        let mut new = **cs as i16 + v * r * f;
        if new < self.min_cache_size as i16 { new = self.min_cache_size as i16; }
        if new > self.max_cache_size as i16 { new = self.max_cache_size as i16; }
        **cs = new as u16;
        new as u16
    }

    fn adjust_cache_size_ex(&self,cs:&mut MutexGuard<u16>,r:u16) ->u16
    {
        let mut new = **cs + r;
        if new < self.min_cache_size  { new = self.min_cache_size; }
        if new > self.max_cache_size  { new = self.max_cache_size; }
        **cs = new;
        new
    }

    async fn send_cache_empty(&self) ->bool
    {
        let queue = self.queue.lock().await;
        queue.len() < self.get_cache_size().await as usize
    }

    fn send_cache_empty_ex(
        queue:&MutexGuard<'_,VecDeque<(usize,Option<u32>)>>,
        msg_cache_queue:&MutexGuard<'_,VecDeque<(usize,Vec<u8>,Option<u32>)>>,
        cache_size:u16) ->bool
    {
        queue.len() < cache_size as usize && msg_cache_queue.is_empty()
    }

    async fn send_close_session(&self) ->Result<(),USErr>
    {
        let sid_ = self.get_sid().await.unwrap();
        let v = Self::warp_ex(&[199],SpecialExt::send_close.into(),TOKEN_NORMAL,0,sid_,None);
        {
            let mut sid = self.sid.lock().await;
            *sid = SessionState::Closed;
        }
        self.send(v.as_slice()).await?;
        self.send(v.as_slice()).await?;
        self.send(v.as_slice()).await?;
        Ok(())
    }
}

#[async_trait]
impl UdpSender for DefUdpSender
{
    async fn send_msg(&self, v: Vec<u8>) -> Result<(),USErr> {
        let mut msg_split = self.msg_split.lock().await;
        {
            let sid = self.sid.lock().await;
            match *sid {
                SessionState::Null => {
                    drop(sid);
                    msg_split.push_msg(v);
                    self.send_session().await?;
                    return Ok(());
                }
                SessionState::WaitResponse(_, _, _) => {
                    drop(sid);
                    msg_split.push_msg(v);
                    return Ok(());
                }
                SessionState::WaitResponseCp(_, _, _) => {}
                SessionState::Closed => {
                    return Err(USErr::AlreadyClosed);
                }
                Has(_) => {}
            }
        }
        msg_split.push_msg(v);

        Ok(())
    }

    async fn check_recv(&self, data: &[u8]) -> Result<(),USErr> {
        self.check_err().await?;
        match self.unwarp(data).await
        {
            Ok(v) => {
                let mut recv_queue = self.recv_queue.lock().await;
                recv_queue.push_back(v);
                Ok(())
            }
            Err(USErr::EmptyMsg) => {Ok(())}
            Err(e) => {
                eprintln!("set error {:?} ",e);
                self.set_error(e.clone()).await;
                Err(e)
            }
        }
    }

    async fn pop_recv_msg(&self) -> Result<Vec<u8>,USErr>
    {
        self.check_err().await?;
        let mut recv_queue = self.recv_queue.lock().await;
        if let Some(v) = recv_queue.pop_front()
        {
            Ok(v)
        }else {
            Err(USErr::EmptyMsg)
        }
    }


    async fn need_check(&self) -> bool {
        self.need_check().await
    }

    fn create(sock: Arc<UdpSocket>,addr:SocketAddr) -> Self {
        let max_cache_size = 1;
        ///消息分割器 的子协议头所占的字节数
        const sub_head_size:usize = size_of::<u128>() + size_of::<u32>() * 3;
        let max_len = 65500 - (Self::package_len() + sub_head_size);
        let min_len = 1472 - (Self::package_len() + sub_head_size);
        DefUdpSender{
            addr,
            sock,
            max_len: max_len as _,
            min_len: min_len as _,
            max_cache_size,
            min_cache_size:1,
            cache_size: Arc::new(Mutex::new(1)),
            mid: Arc::new(Mutex::new(usize::zero())),
            queue: Arc::new(Mutex::new(VecDeque::new())),
            msg_map: Arc::new(Mutex::new(HashMap::new())),
            expect_id: Arc::new(Mutex::new(1)),
            recv_cache: Arc::new(Mutex::new(HashMap::new())),
            subpacker: Arc::new(Mutex::new(UdpSubpackage::new())),
            timeout: Duration::from_millis(400),
            msg_split: Arc::new(Mutex::new(UdpMsgSplit::with_max_unit_size(max_len,min_len))),
            max_retry_times: 6,
            recv_queue: Arc::new(Mutex::new(VecDeque::new())),
            error :Arc::new(Mutex::new(None)),
            sid: Arc::new(Mutex::new(SessionState::Null)),
            adjust_cache_size_time: Arc::new(Mutex::new(SystemTime::now())),
            avg_retry_times: Arc::new(Mutex::new((0,0f32))),
            // data_current_limiter: Arc::new(DataCurrentLimiter::new(
            //     Duration::from_millis(20),
            //     (1024*1024*4) / (1000/20) ,10,10
            // ))
        }
    }

    async fn set_max_msg_len(&mut self, len: u16) {
        self.max_len = len;
        let mut msg_split = self.msg_split.lock().await;
        msg_split.set_max_unit_size(len as _);
    }

    fn max_msg_len(&self) -> u16 {
        self.max_len
    }

    async fn set_min_msg_len(&mut self, len: u16) {
        self.min_len = len;
        let mut msg_split = self.msg_split.lock().await;
        msg_split.set_min_unit_size(len as _)
    }

    fn min_msg_len(&self) -> u16 {
        self.min_len
    }

    fn min_cache_size(&self) -> u16 {
        self.min_cache_size
    }

    fn set_min_cache_size(&mut self, s: u16) {
        self.min_cache_size = s;
    }

    fn max_cache_size(&self) -> u16 {
        self.max_cache_size
    }

    fn set_max_cache_size(&mut self, s: u16) {
        self.max_cache_size = s;
    }

    fn set_time_out(&mut self, dur: Duration) {
        self.timeout = dur;
    }

    async fn check_send(&self) -> Result<(),USErr> {
        self.check_err().await?;
        let now = SystemTime::now();
        {
            let mut sid = self.sid.lock().await;

            if let SessionState::WaitResponse(v,t,times) = *sid {
                if times > self.max_retry_times {
                    return Err(USErr::SendSessionFailed);
                }
                if let Ok(dur) = SystemTime::now().duration_since(t)
                {
                    if dur > self.timeout
                    {
                        let d = Self::warp_ex(&[199],SpecialExt::send_sid.into(),TOKEN_NORMAL,0,v,None);
                        self.send(d.as_slice()).await?;
                        *sid = SessionState::WaitResponse(v,SystemTime::now(),times + 1);
                    }
                }
            }
            if let SessionState::WaitResponseCp(v,t,times) = *sid {
                if times > self.max_retry_times {
                    return Err(USErr::SendSessionFailed);
                }
                if let Ok(dur) = SystemTime::now().duration_since(t)
                {
                    if dur > self.timeout
                    {
                        let d = Self::warp_ex(&[199],SpecialExt::response_send_sid.into(),TOKEN_NORMAL,0,v,None);
                        self.send(d.as_slice()).await?;
                        *sid = SessionState::WaitResponseCp(v,SystemTime::now(),times + 1);
                    }
                }
            }
        }
        let mut avg_times = 0f32;
        {
            let queue = self.queue.lock().await;
            let mut msg_map = self.msg_map.lock().await;
            let mut l = 0;
            let mut sum = 0;
            for (id,_) in queue.iter(){
                if let Some((v,t,times)) = msg_map.get_mut(id){
                    if l >= self.get_cache_size().await {
                        break;
                    }
                    sum += *times;
                    if *times > self.max_retry_times {
                        eprintln!("msg {} retry times reach the maximum!", id);
                        return Err(USErr::RetryTimesLimit);
                    }
                    if let Ok(dur) = now.duration_since(*t)
                    {
                        if dur > self.timeout * *times as u32
                        {
                            //if self.data_current_limiter.can_send(v.len()).await
                            {
                                *t = SystemTime::now();
                                *times += 1;
                                println!("send {} times {} len {}",*id,times,v.len());
                                Self::send_ex(self.sock.clone(), self.addr, v.as_slice()).await?;
                            }
                        }
                    }
                    l += 1;
                }
            }
            if l > 0 { avg_times = sum as f32 / l as f32; }
        }
        if self.has_sid().await {
            let mut msg_split = self.msg_split.lock().await;
            while self.send_cache_empty().await && msg_split.need_send() {
                //println!("pop msg");
                if let Some((v,ext,tag,msg_slices_info)) = msg_split.pop_msg(){
                    let (sub_head,rid) = match msg_slices_info{
                        MsgSlicesInfo::Complete => {  (None,None)  }
                        MsgSlicesInfo::Part(ref v) => { (Some(v.as_slice()),Some(ext)) }
                    };
                    match self.warp(v,ext,tag, sub_head,rid).await{
                        Ok(v) => {
                            //if self.data_current_limiter.can_send(v.len()).await
                            {
                                self.send(v.as_slice()).await?;
                            }
                        }
                        Err(USErr::MsgCacheOverflow) => {}
                        Err(e) => { return Err(e);}
                    }
                }else{break;}
            }
        }

        let mut adjust_cache_size_time = self.adjust_cache_size_time.lock().await;
        if let Ok(dur) = SystemTime::now().duration_since(*adjust_cache_size_time) {
            let mut retry_times = self.avg_retry_times.lock().await;
            (*retry_times).0 += 1;
            (*retry_times).1 += avg_times;
            if dur.as_secs_f32() >  self.timeout.as_secs_f32() * 5f32 {
                let avg =  (*retry_times).1 / (*retry_times).0 as f32;
                self.adjust_unit_size(avg).await;
                *retry_times = (0,0f32);
                *adjust_cache_size_time = SystemTime::now();
            }
        }
        //self.check_msg_cache_queue().await;
        Ok(())
    }

    fn set_retry_times(&mut self, v: u16) {
        self.max_retry_times = v;
    }

    fn retry_times(&self) -> u16 {
        self.max_retry_times
    }

    async fn has_session(&self) -> bool {
        self.has_sid().await
    }

    async fn build_session(&self) -> Result<(), USErr> {
        if self.has_sid().await {
            return Err(USErr::AlreadyHasSession);
        }
        if self.is_waiting_session().await {
            return Err(USErr::WaitSessionResponse);
        }
        self.send_session().await?;
        Ok(())
    }

    async fn close_session(&self) -> Result<(), USErr> {
        if !self.has_session().await
        {
            return Err(USErr::NoSession);
        }
        self.send_close_session().await
    }
}




