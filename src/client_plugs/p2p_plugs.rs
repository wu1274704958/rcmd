use rcmd_suit::client_plug::client_plug::ClientPlug;
use std::sync::{Arc};
use tokio::net::UdpSocket;
use rcmd_suit::utils::udp_sender::USErr;
use async_trait::async_trait;
use std::net::SocketAddr;
use tokio::sync::Mutex;


struct PlugData{
    socket:Option<Arc<UdpSocket>>,
    port: u16
}

impl PlugData {
    pub fn new() -> PlugData
    {
        PlugData{
            socket : None,
            port : 0
        }
    }
}

pub struct P2PPlug{
    data : Arc<Mutex<PlugData>>
}

impl P2PPlug {
    pub fn new()-> P2PPlug
    {
        P2PPlug{
            data : Arc::new(Mutex::new(PlugData::new())),
        }
    }
}

#[async_trait]
impl ClientPlug for P2PPlug {
    type SockTy = UdpSocket;
    type ErrTy = USErr;

    async fn on_init(&self) {

    }

    async fn on_create_socket(&self, sock: Arc<Self::SockTy>) {
        let mut d = self.data.lock().await;
        d.socket = Some(sock.clone());
    }

    async fn on_get_local_addr(&self, addr: SocketAddr) {
        dbg!(addr);
        let mut d = self.data.lock().await;
        d.port = addr.port();
    }

    async fn on_get_err(&self, err: Self::ErrTy) where Self::ErrTy: Clone {

    }

    async fn on_lauch_recv_worker(&self) {

    }

    async fn on_stop(&self) {

    }

    async fn on_recv_oth_msg(&self, addr: SocketAddr, data: &[u8]) {
        println!("recv other msg from {:?}\n{:?}",&addr,data);
    }

    async fn on_lauch_loop(&self) {

    }

    fn capture(&self, ext:u32) -> bool {
        false
    }
}