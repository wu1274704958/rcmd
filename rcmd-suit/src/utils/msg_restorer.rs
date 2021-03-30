trait MsgRestorer{
    fn need_merge(&self, tag:u8) -> bool;
    fn merge(&mut self, msg:&[u8],ext:u32,tag:u8) -> Option<Vec<u8>>;
}



trait MsgSpliter{
    fn open(&self)->bool;
    fn push_msg(&self,v:Vec<u8>);
    
    fn with_max_unit_size(max_unit_size:usize,min_unit_size:usize)->Self;
    fn is_max_unit_size(&self)->bool;
    fn is_min_unit_size(&self)->bool;
    fn up_unit_size(&mut self);
    fn down_unit_size(&mut self);
    fn max_unit_size(&self);
    fn set_max_unit_size(&mut self, max_unit_size: usize);
    fn set_min_unit_size(&mut self, min_unit_size: usize);
    fn min_unit_size(&self) -> usize;
    fn unit_size(&self) -> usize;
}