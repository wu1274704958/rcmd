use std::sync::{Arc};
use tokio::sync::Mutex;
use std::collections::HashMap;
use async_trait::async_trait;


#[async_trait]
pub trait Plug :Send + Sync{
    type ABClient;
    type Id;
    type Config;

    async fn run(&self,id:Self::Id,clients:&Arc<Mutex<HashMap<Self::Id,Box<Self::ABClient>>>>,config:Arc<Self::Config>) where Self::Id :Copy;
}
#[async_trait]
pub trait PlugMgr<T> where T:Plug
{
    async fn run(&self,
        id:<T as Plug>::Id,
        clients:&Arc<Mutex<HashMap<<T as Plug>::Id,Box<<T as Plug>::ABClient>>>>,
        config:Arc<<T as Plug>::Config>) where <T as Plug>::Id :Copy+Send,
    <T as Plug>::ABClient :Send,
    <T as Plug>::Config : Send + Sync,
    T : 'async_trait
    {
        for i in 0..self.plug_count()
        {
            let p = self.get_plug(i);
            p.run(id,clients,config.clone()).await;
        }
    }

    fn add_plug(&mut self,h:Arc<dyn Plug<ABClient = <T as Plug>::ABClient, Id = <T as Plug>::Id,Config = <T as Plug>::Config>>);
    fn plug_count(&self)->usize;
    fn get_plug(&self,i:usize)->&dyn Plug<ABClient = <T as Plug>::ABClient, Id = <T as Plug>::Id,Config = <T as Plug>::Config>;
}

pub struct DefPlugMgr<T> where T:Plug {
    plugs : Vec<Arc<dyn Plug<ABClient = <T as Plug>::ABClient, Id = <T as Plug>::Id,Config = <T as Plug>::Config>>>
}

impl<T> DefPlugMgr<T>  where T:Plug {
    pub fn new()->DefPlugMgr<T>
    {
        DefPlugMgr::<T>{
            plugs:Vec::new()
        }
    }
}
#[async_trait]
impl<T> PlugMgr<T> for DefPlugMgr<T> where T:Plug{
    fn add_plug(&mut self, h: Arc<dyn Plug<ABClient=<T as Plug>::ABClient, Id=<T as Plug>::Id, Config=<T as Plug>::Config>>) {
        self.plugs.push(h);
    }

    fn plug_count(&self) -> usize {
        self.plugs.len()
    }

    fn get_plug(&self, i: usize) -> &dyn Plug<ABClient=<T as Plug>::ABClient, Id=<T as Plug>::Id, Config=<T as Plug>::Config> {
        self.plugs[i].as_ref()
    }
}


