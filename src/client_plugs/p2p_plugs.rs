use rcmd_suit::client_plug::client_plug::ClientPlug;
use std::sync::Arc;
use tokio::net::UdpSocket;
use rcmd_suit::utils::udp_sender::USErr;
use async_trait::async_trait;
use std::net::SocketAddr;

pub struct P2PPlug{
    
}

#[async_trait]
impl ClientPlug for P2PPlug {
    type SockTy = UdpSocket;
    type ErrTy = USErr;

    async fn on_init(&self) {

    }

    async fn on_create_socket(&self, sock: Arc<Self::SockTy>) {
        if let Ok(addr) = sock.local_addr()
        {
            dbg!(&addr);
        }
    }

    async fn on_get_err(&self, err: Self::ErrTy) where Self::ErrTy: Clone {

    }

    async fn on_begin(&self) {

    }

    async fn on_stop(&self) {

    }

    async fn on_recv_oth_msg(&self, addr: SocketAddr, data: &[u8]) {

    }
}