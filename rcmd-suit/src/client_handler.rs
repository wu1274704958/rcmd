use std::sync::Arc;
use crate::agreement::Message;
use async_trait::async_trait;
#[async_trait]
pub trait SubHandle:Send + Sync {
    async fn handle(&self,data:&[u8],len:u32,ext:u32)-> Option<(Vec<u8>,u32)>;
    fn interested(&self,ext:u32)->bool{true} 
}
#[async_trait]
pub trait Handle
{
    async fn handle_ex(&self,data:Message<'_>
                 )-> Option<(Vec<u8>,u32)>
    {
        for i in 0..self.handler_count()
        {
            let handler = self.get_handler(i);
            if handler.interested(data.ext) {
                if let Some((v,ext)) = handler.handle(data.msg,data.len,data.ext).await
                {
                    return Some((v,ext));
                }
            }
        }
        None
    }

    fn add_handler(&mut self,h:Arc<dyn SubHandle>);
    fn handler_count(&self)->usize;
    fn get_handler(&self,i:usize)->&dyn SubHandle;
}


pub struct DefHandler{
    handlers : Vec<Arc<dyn SubHandle>>
}

impl DefHandler {
    pub fn new()->DefHandler
    {
        DefHandler{
            handlers:Vec::new()
        }
    }
}

impl Handle for DefHandler{

    fn add_handler(&mut self, h: Arc<dyn SubHandle>) {
        self.handlers.push(h);
    }

    fn handler_count(&self) -> usize {
        self.handlers.len()
    }

    fn get_handler(&self, i: usize) -> &dyn SubHandle {
        self.handlers[i].as_ref()
    }

}


