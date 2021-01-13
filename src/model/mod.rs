pub mod user;

use serde::{Serialize, Deserialize};

#[derive(Clone,Debug,PartialEq,Eq, Serialize, Deserialize)]
pub struct SendMsg{
    pub lid:usize,
    pub msg:String
}

#[derive(Clone,Debug,PartialEq,Eq, Serialize, Deserialize)]
pub struct RecvMsg{
    pub lid:usize,
    pub msg:String,
    pub from_name:String
}

#[derive(Clone,Debug,PartialEq,Eq, Serialize, Deserialize)]
pub struct PullFileMsg{
    pub far_end_path:String,
    pub near_end_path:Option<String>
}

