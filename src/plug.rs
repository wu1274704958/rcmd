use std::sync::{Arc, Mutex};
use std::collections::HashMap;

pub trait Plug :Send + Sync{
    type ABClient;
    type Id;
    type Config;

    fn run(&self,id:Self::Id,clients:&Arc<Mutex<HashMap<Self::Id,Box<Self::ABClient>>>>,config:&Self::Config) where Self::Id :Copy;
}

pub trait PlugMgr<T> where T:Plug
{
    fn run(&self,
        id:<T as Plug>::Id,
        clients:&Arc<Mutex<HashMap<<T as Plug>::Id,Box<<T as Plug>::ABClient>>>>,
        config:&<T as Plug>::Config) where <T as Plug>::Id :Copy
    {
        for i in 0..self.plug_count()
        {
            let p = self.get_plug(i);
            p.run(id,clients,config);
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


