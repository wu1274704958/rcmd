use rcmd_suit::handler::{SubHandle, Handle};
use rcmd_suit::plug::{Plug, PlugMgr};
use rcmd_suit::config_build::Config;
use rcmd_suit::agreement::Agreement;
use std::ops::AddAssign;
use rcmd_suit::ab_client::{ABClient, State};
use std::net::SocketAddr;
use tokio::net::UdpSocket;
use std::sync::Arc;
use async_std::channel::Receiver;
use rcmd_suit::servers::udp_server::{UdpServer, run_in};
use tokio::sync::Mutex;
use std::collections::{HashMap, VecDeque};
use rcmd_suit::utils::udp_sender::{UdpSender, DefUdpSender};
use ahash::{CallHasher, RandomState};
use std::hash::BuildHasher;
use crate::client_plugs::attched_udp_sender::AttchedUdpSender;
use crate::EXT_P2P_RELAY_MSG_CS;

#[allow(unused_must_use)]
pub async fn run_udp_server_with_channel<LID,ABC,P,SH,H,PL,PLM>(
    rx:Receiver<(SocketAddr,Vec<u8>)>,
    sock: Arc<UdpSocket>,
    server:UdpServer<LID,ABC,P,SH,H,PL,PLM>,
    msg_split_ignore:Option<&'static Vec<u32>>,
    asy_cry_ignore:Option<&'static Vec<u32>>,
    relay_map: Arc<Mutex<HashMap<SocketAddr,usize>>>,
    curr_sender: Arc<Mutex<VecDeque<(Vec<u8>, u32)>>>,
) -> Result<(), Box<dyn std::error::Error>>
    where SH : SubHandle<ABClient=ABC,Id=LID> + 'static,
          H : Handle<SH> + Send + std::marker::Sync + 'static,
          PL : Plug<ABClient=ABC,Id=LID,Config=Config> + 'static,
          PLM: PlugMgr<PL> + Send + std::marker::Sync + 'static,
          P : Agreement + Send + std::marker::Sync + 'static,
          LID : AddAssign + Clone + Copy + Eq + std::hash::Hash+num_traits::identities::Zero + num_traits::identities::One + Send + std::marker::Sync + 'static,
          ABC: ABClient<LID = LID> + Send + 'static

{
    let linker_map = Arc::new(Mutex::new(HashMap::<u64,Arc<dyn UdpSender>>::new()));
    let hash_builder = RandomState::new();

    loop {

        match rx.recv().await{
            Ok((addr,buf)) =>{
                let id = CallHasher::get_hash(&addr, hash_builder.build_hasher());
                let has = {
                    let map = linker_map.lock().await;
                    if let Some(link) = map.get(&id){
                        link.check_recv(&buf[..]).await;
                        while link.need_check().await { link.check_recv(&[]).await; }
                        true
                    }else { false }
                };
                if !has
                {
                    let sender:Arc<dyn UdpSender> = {
                        let relay_map = relay_map.lock().await;
                        if let Some(cpid) = relay_map.get(&addr)
                        {
                            println!("Create Relay Sender");
                            Arc::new(AttchedUdpSender::new(curr_sender.clone(),
                                                           EXT_P2P_RELAY_MSG_CS,*cpid))
                        }else{
                            drop(relay_map);
                            Arc::new(DefUdpSender::New(sock.clone(),addr))
                        }
                    };

                    {
                        sender.check_recv(&buf[..]).await;
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
                        let on_ret = async move{
                            let id_ = id;
                            let mut map = linker_map_cp.lock().await;
                            map.remove(&id_);
                            println!("disconnect id = {}",id_);
                        };
                        server.runtime.spawn(run_in(
                            clients,lid,conf,handler_cp,parser_cp,plugs_cp,dead_plugs_cp,sock_cp,sender,
                            addr, on_ret,asy_cry_ignore.clone(),msg_split_ignore.clone()
                        ));

                        println!("Spawn a client handler!!!");
                    }
                }
            }
            Err(e) => {
                eprintln!("err = {:?}",e);
                let mut cls =  server.clients.lock().await;
                for c in cls.values_mut(){
                    c.set_state(State::WaitKill);
                }
                break;
            }
        }
    }

    server.runtime.shutdown_background();

    Ok(())
}