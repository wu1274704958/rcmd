use tokio::time::Duration;
use tokio::time::sleep;

mod context;
mod client;
mod model;
mod client_plugs;


fn main() {
    println!("===");
    let test = test();
    tokio::spawn(test);
}

async fn test()
{
    sleep(Duration::from_secs(1)).await;
    println!("---");
}