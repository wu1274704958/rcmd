use async_trait::async_trait;

#[async_trait]
pub trait P2PEvent : Sync + Send{
    async fn on_recv_p2p_req(&self,cpid:usize);
    async fn on_recv_wait_accept_timeout(&self,cpid:usize);
}