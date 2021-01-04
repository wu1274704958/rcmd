use async_std::sync::Arc;
use std::sync::Mutex;
use std::collections::HashMap;
use crate::handler::SubHandle;
use std::collections::hash_map::RandomState;
use crate::ab_client::AbClient;
use crate::ext_code::{EXT_RUN_CMD, EXT_EXEC_CMD};
use std::ops::AddAssign;

pub enum ExecState{
    Init,
    SendToTarget,
    Received,
    SendToSu
}

pub struct ExecData{
    pub tar:usize,
    pub su:usize,
    pub st:ExecState
}

impl ExecData{
    pub fn new(tar:usize,su:usize)->ExecData
    {
        ExecData{
            tar,
            su,
            st:ExecState::Init
        }
    }
}

pub struct ExecCmd{
    logic_id:Arc<Mutex<usize>>,
    data:Arc<Mutex<HashMap<usize,ExecData>>>
}

impl ExecCmd{
    pub fn new()->ExecCmd{
        ExecCmd{
            logic_id : Arc::new(Mutex::new(0)),
            data : Arc::new(Mutex::new(HashMap::new()))
        }
    }

    pub fn next_id(&self) ->usize
    {
        if let Ok(mut l) = self.logic_id.lock(){
            l.add_assign(1);
            return *l;
        }
        0
    }

    pub fn del_data(&self,i:usize)
    {
        if let Ok(mut d) = self.data.lock(){
            if d.contains_key(&i) {
                d.remove(&i);
            }
            if d.len() == 0
            {
                if let Ok(mut l) = self.logic_id.lock(){
                    l.set_zero();
                }
            }
        }
    }

    pub fn set_st(&self,i:usize,st:ExecState)
    {
        if let Ok(mut d) = self.data.lock(){
            if d.contains_key(&i) {
                d.get_mut(&i).unwrap().st = st;
            }
        }
    }
}

impl SubHandle for ExecCmd
{
    type ABClient = AbClient;
    type Id = usize;

    fn handle(&self, data: &[u8], len: u32, ext: u32, clients: &Arc<Mutex<HashMap<Self::Id, Box<Self::ABClient>, RandomState>>>, id: Self::Id) -> Option<(Vec<u8>, u32)> where Self::Id: Copy {
        if ext == EXT_RUN_CMD{

        }else if ext == EXT_EXEC_CMD{

        }
        None
    }
}