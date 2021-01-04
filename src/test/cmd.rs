use std::process::{Command, Output, Stdio};
use tokio::io::Error;
use std::io::{Stdout, Stderr, Write, Read};
use std::thread::sleep;
use tokio::time::Duration;

fn main()
{
    let mut c = Command::new("C:\\Windows\\System32\\cmd.exe")
        .arg("dir")
        .stdout(Stdio::piped())
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn().unwrap();

    {
        if let Some(ref mut i) = c.stdin{
            i.write_all(b"dir/r/n").unwrap();
        }
    }
    {
        dbg!(c.wait());
    }
    let mut s = String::new();
    c.stdout.unwrap().read_to_string(&mut s);
    dbg!(s);

    let mut s = String::new();
    c.stderr.unwrap().read_to_string(&mut s);
    dbg!(s);
}