use std::sync::Arc;
use tokio::sync::{Mutex, MutexGuard};
use std::collections::{VecDeque, HashMap};
use rcmd_suit::subpackage::{UdpSubpackage, Subpackage};
use rcmd_suit::utils::udp_sender::{USErr, SessionState, SpecialExt, UdpSender};
use rcmd_suit::tools::TOKEN_NORMAL;
use std::time::{SystemTime, Duration};
use std::convert::TryFrom;
use std::mem::size_of;
use rcmd_suit::tools;
use rcmd_suit::utils::msg_split::{DefUdpMsgSplit, UdpMsgSplit, MsgSlicesInfo};
use num_traits::identities::One;
use num_traits::Zero;
use tokio::net::UdpSocket;
use std::net::SocketAddr;
use std::process::abort;
use async_trait::async_trait;

pub struct AttchedUdpSender {
    send_queue: Arc<Mutex<VecDeque<(Vec<u8>,u32)>>>,
    send_ext: u32,
    cpid: usize,
    mid: Arc<Mutex<usize>>,
    recv_cache: Arc<Mutex<HashMap<usize,(Vec<u8>,u32,u8,Option<Vec<u8>>)>>>,
    expect_id: Arc<Mutex<usize>>,
    subpacker: Arc<Mutex<UdpSubpackage>>,
    recv_queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
    sid: Arc<Mutex<SessionState>>,
    error: Arc<Mutex<Option<USErr>>>,
    msg_split: Arc<Mutex<DefUdpMsgSplit>>,
    timeout: Duration,
    max_retry_times: u16,
}

impl AttchedUdpSender {
    pub fn new(
        send_queue: Arc<Mutex<VecDeque<(Vec<u8>,u32)>>>,
        send_ext: u32,
        cpid: usize
    ) -> AttchedUdpSender {
        const sub_head_size:usize = size_of::<u128>() + size_of::<u32>() * 3;
        let max_len = 65500 - (Self::package_len() + sub_head_size);
        let min_len = 1472 - (Self::package_len() + sub_head_size);
        let mut msg_split = DefUdpMsgSplit::with_max_unit_size(max_len,min_len);
        msg_split.set_unit_size(usize::max_value());

        AttchedUdpSender {
            send_queue,
            send_ext,
            cpid,
            mid: Arc::new(Mutex::new(usize::zero())),
            expect_id: Arc::new(Mutex::new(1)),
            recv_cache: Arc::new(Mutex::new(HashMap::new())),
            subpacker: Arc::new(Mutex::new(UdpSubpackage::new())),
            recv_queue: Arc::new(Mutex::new(VecDeque::new())),
            sid: Arc::new(Mutex::new(SessionState::Null)),
            error: Arc::new(Mutex::new(None)),
            msg_split: Arc::new(Mutex::new(msg_split)),
            timeout: Duration::from_millis(500),
            max_retry_times: 9
        }
    }

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
        //self.push_cache(mid,res.clone(), rid).await?;
        //self.send(res.as_slice()).await?;
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
            if ext > SpecialExt::err_beigin.into() && ext < SpecialExt::err_end.into()
            {
                return self.handle_special_err(sp_ext).await;
            }
            if ext >= SpecialExt::req_close.into() && ext <= SpecialExt::close_succ.into()
            {
                return self.handle_special_close(sp_ext,id,sid).await;
            }
            match sp_ext {
                SpecialExt::send_recv => {
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
                        SessionState::Closed(_) |
                        SessionState::PrepareClose(_,_,_) |
                        SessionState::ReqClose(_,_,_) => {
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
                            }else {
                                self.send_sid_response(sid, SpecialExt::response_cp_send_sid).await?;
                            }
                        }
                        SessionState::Closed(_) |
                        SessionState::PrepareClose(_,_,_) |
                        SessionState::ReqClose(_,_,_) => {
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
                        SessionState::Closed(_)|
                        SessionState::PrepareClose(_,_,_) |
                        SessionState::ReqClose(_,_,_) => {
                            self.send_ext(SpecialExt::err_already_closed).await?;
                        }
                    }
                    Ok(true)
                }
                _ => { Ok(false) }

            }
        }else{
            Ok(false)
        }
    }

    async fn handle_special_err(&self,ext:SpecialExt) -> Result<bool,USErr>
    {
        match ext
        {
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
            SpecialExt::err_bad_request => {
                Err(USErr::BadRequest)
            }
            _ => {
                Ok(false)
            }
        }
    }

    async fn clear_send_queue(&self)
    {
        let mut queue = self.send_queue.lock().await;
        queue.clear();
    }

    async fn handle_special_close(&self,ext:SpecialExt,id:usize,sid:u128) -> Result<bool,USErr>
    {
        match ext
        {
            SpecialExt::req_close => {
                let mut sid_ = self.sid.lock().await;
                match *sid_ {
                    SessionState::Has(v) => {
                        if sid != v {
                            self.send_ext(SpecialExt::err_bad_session).await?;
                        }else {
                            println!("cp request close!");
                            *sid_ = SessionState::PrepareClose(sid,SystemTime::now(),1);
                            drop(sid);
                            self.clear_send_queue().await;
                            self.send_sid_response(sid, SpecialExt::cp_close).await?;
                        }
                    }
                    SessionState::Null |
                    SessionState::WaitResponse(_,_,_)|
                    SessionState::WaitResponseCp(_,_,_)=> {
                        self.send_ext(SpecialExt::err_no_session).await?;
                    }
                    _ => {}
                }
                Ok(true)
            }
            SpecialExt::cp_close =>{
                let mut sid_ = self.sid.lock().await;
                match *sid_ {
                    SessionState::ReqClose(v,_,_) => {
                        if sid != v{
                            self.send_ext(SpecialExt::err_bad_session).await?;
                        }else{
                            println!("cp response close!");
                            *sid_ = SessionState::Closed(sid);
                            drop(sid);
                            self.send_sid_response(sid, SpecialExt::close_succ).await?;
                            return Err(USErr::CloseSuccess);
                        }
                    }
                    SessionState::Closed(v) =>{
                        if sid != v{
                            self.send_ext(SpecialExt::err_bad_request).await?;
                        }else{
                            self.send_sid_response(sid, SpecialExt::close_succ).await?;
                        }
                    }
                    _ => {
                        self.send_ext(SpecialExt::err_bad_request).await?;
                    }
                }
                Ok(true)
            }
            SpecialExt::close_succ =>{
                let mut sid_ = self.sid.lock().await;
                match *sid_ {
                    SessionState::PrepareClose(v,_,_) => {
                        if sid != v{
                            self.send_ext(SpecialExt::err_bad_session).await?;
                        }else{
                            println!("cp close success close self!");
                            *sid_ = SessionState::Closed(sid);
                            drop(sid);
                            return Err(USErr::CloseSuccess);
                        }
                    }
                    SessionState::Closed(v) => {
                        if sid != v{
                            self.send_ext(SpecialExt::err_bad_request).await?;
                        }
                    }
                    _ => {
                        self.send_ext(SpecialExt::err_bad_request).await?;
                    }
                }
                Ok(true)
            }
            _ => {
                Ok(false)
            }
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
        self.send(v.as_slice()).await
    }

    async fn send(&self,msg:&[u8]) -> Result<usize,USErr>
    {
        let mut d = Vec::with_capacity(msg.len() + size_of::<usize>());
        d.extend_from_slice(self.cpid.to_be_bytes().as_ref());
        d.extend_from_slice(msg);
        let mut queue = self.send_queue.lock().await;
        queue.push_back((d,self.send_ext));
        Ok(msg.len())
    }

    async fn send_ex(send_queue:Arc<Mutex<VecDeque<(Vec<u8>,u32)>>>,ext:u32,cpid:usize,msg:&[u8]) -> Result<usize,USErr>
    {
        let mut d = Vec::with_capacity(msg.len() + size_of::<usize>());
        d.extend_from_slice(cpid.to_be_bytes().as_ref());
        d.extend_from_slice(msg);
        let mut queue = send_queue.lock().await;
        queue.push_back((d,ext));
        Ok(msg.len())
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
            SessionState::Closed(_) |
            SessionState::PrepareClose(_,_,_) |
            SessionState::ReqClose(_,_,_) => {None}
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
            SessionState::Closed(_)|
            SessionState::PrepareClose(_,_,_) |
            SessionState::ReqClose(_,_,_) => {false}
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

    async fn send_close_session(&self) ->Result<(),USErr>
    {
        let sid_ = self.get_sid().await.unwrap();
        let v = Self::warp_ex(&[199],SpecialExt::req_close.into(),TOKEN_NORMAL,0,sid_,None);
        {
            let mut sid = self.sid.lock().await;
            *sid = SessionState::ReqClose(sid_,SystemTime::now(),1);
        }
        self.clear_send_queue().await;
        self.send(v.as_slice()).await?;
        Ok(())
    }
}

#[async_trait]
impl UdpSender for AttchedUdpSender
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
                SessionState::Closed(_)|
                SessionState::PrepareClose(_,_,_) |
                SessionState::ReqClose(_,_,_) => {
                    return Err(USErr::AlreadyClosed);
                }
                SessionState::Has(_) => {}
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
        abort()
    }

    async fn set_max_msg_len(&mut self, len: u16) {}

    fn max_msg_len(&self) -> u16 {
        0
    }

    async fn set_min_msg_len(&mut self, len: u16) {}

    fn min_msg_len(&self) -> u16 {
        0
    }

    fn min_cache_size(&self) -> u16 {0}

    fn set_min_cache_size(&mut self, s: u16) {
    }

    fn max_cache_size(&self) -> u16 {0 }

    fn set_max_cache_size(&mut self, s: u16) {}

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
        if self.has_sid().await {
            let mut msg_split = self.msg_split.lock().await;
            while msg_split.need_send() {
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