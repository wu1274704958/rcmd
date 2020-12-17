use async_std::task;
use std::time::SystemTime;
use std::thread::sleep;
use tokio::time::Duration;
use std::ops::Sub;

async fn say_hi() {
    println!("Hello, world!");
}

fn main() {


    let a = SystemTime::now();
    sleep(Duration::from_millis(2));
    let b = SystemTime::now();

    dbg!(b.duration_since(a));

    task::block_on(say_hi())
}