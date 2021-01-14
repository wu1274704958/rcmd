use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

#[derive(Debug,Clone)]
pub struct TempPermission{
    map:Arc<Mutex<HashMap<usize,HashSet<usize>>>>
}

impl TempPermission{
    pub fn new() -> TempPermission
    {
        TempPermission{
            map:Arc::new(Mutex::new(HashMap::new()))
        }
    }

    pub fn give_permission(&self,m:usize,n:usize)
    {
        if let Ok(mut map) = self.map.lock()
        {
            if let Some(set) = map.get_mut(&m)
            {
                if !set.contains(&n)
                {
                    set.insert(n);
                }
            }else{
                let mut set = HashSet::new();
                set.insert(n);
                map.insert(m,set);
            }
        }
    }

    pub fn take_permission(&self,m:usize,n:usize)
    {
        if let Ok(mut map) = self.map.lock()
        {
            if let Some(set) = map.get_mut(&m)
            {
                if set.contains(&n)
                {
                    set.remove(&n);
                }
            }
        }
    }

    pub fn take_all_permission(&self,m:usize)
    {
        if let Ok(mut map) = self.map.lock()
        {
            if map.contains_key(&m)
            {
                map.remove(&m);
            }
        }
    }

    pub fn has_temp_permission(&self,m:usize,n:usize)->bool
    {
        if let Ok(map) = self.map.lock()
        {
            if let Some(set) = map.get(&m)
            {
                set.contains(&n)
            }else{
                false
            }
        }else{
            false
        }
    }
}