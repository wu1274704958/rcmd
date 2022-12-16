use num_traits::{Num};
use std::cmp::Ordering;
use std::ops::{AddAssign, Deref};
use  misc_gen::gen_cycle_num;
use std::num::Wrapping;

pub trait CycleNum
{
    fn uncheck_add_assign(&mut self,v:Self);
    fn cycle_partial_cmp(&self,v:&Self,r:Self) -> Option<Ordering>;
    fn cycle_cmp(&self,v:&Self,r:Self) -> Ordering ;
    fn in_min_range(&self,v:Self,r:Self) -> bool;
    fn in_max_range(&self,v:Self,r:Self) -> bool;
}
gen_cycle_num!{u8}
gen_cycle_num!{u16}
gen_cycle_num!{u32}
gen_cycle_num!{u64}
gen_cycle_num!{usize}
gen_cycle_num!{u128}

#[derive(Copy,Clone)]
pub struct CycleCount<T>
    where T:Num + Copy + Ord + CycleNum
{
    val:T,
    cycle_range:T
}

impl<T> CycleCount<T>
    where T:Num + Copy + Ord + CycleNum
{
    pub fn new(v:T,r:T) -> CycleCount<T>
    {
        CycleCount::<T>{
            val : v,
            cycle_range : r
        }
    }
}

impl<T> Deref for CycleCount<T>
    where T:Num + Copy + Ord + CycleNum
{
    type Target = T;
    fn deref(&self) -> &Self::Target
    {
        &self.val
    }
}

impl<T> Eq for CycleCount<T>
    where T:Num + Copy + Ord + CycleNum
{}

impl<T> PartialEq<Self> for CycleCount<T>
    where T:Num + Copy + Ord + CycleNum
{
    fn eq(&self, other: &Self) -> bool {
        self.val.eq( &other.val)
    }
}

impl<T> PartialOrd<Self> for CycleCount<T>
    where T:Num + Copy + Ord + CycleNum
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.val.cycle_partial_cmp(&other.val,self.cycle_range)
    }
}

impl<T> AddAssign<T> for CycleCount<T>
    where T:Num + Copy + Ord + CycleNum
{
    fn add_assign(&mut self, rhs: T)
    {
        self.val.uncheck_add_assign(rhs);
    }
}

impl<T> Ord for CycleCount<T>
    where T:Num + Copy + Ord + CycleNum
{
    fn cmp(&self, other: &Self) -> Ordering
    {
        self.val.cycle_cmp(&other.val,self.cycle_range)
    }
}


mod Test{
    use crate::utils::cycle_count::{CycleNum,CycleCount};
    use std::num::Wrapping;
    #[test]
    fn test()
    {
        let mut a = CycleCount::new( 0u32,9999u32);
        let mut b = CycleCount::new(9998u32,9999u32);
        for _ in 0..99999999999u64 {
            a += 2;
            b += 2;
            assert!(a < b,"{}", format!("test failed {} {}",a.val,b.val));
        }
    }
}
