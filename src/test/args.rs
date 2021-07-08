use args::{Args,ArgsError};
use getopts::Occur;
use std::net::Ipv4Addr;
use std::str::FromStr;


#[derive(Debug)]
pub struct ClientArgs{
    pub ip : Ipv4Addr,
    pub port : u16,
    pub acc : Option<String>,
    pub pwd : Option<String>
}

fn main() {
    let args = match parse_c_args() {
        Ok(a) => {
            a
        }
        Err(e) => {
            dbg!(e);
            return;
        }
    };
    dbg!(args);
}

fn parse_c_args() -> Result<ClientArgs, ArgsError> {
    let input:Vec<_> = std::env::args().collect();
    let mut args = Args::new("Client", "-=-=-=-=-=-=-=-=-=-=-");
    args.flag("h", "help", "Print the usage menu");
    args.option("i",
                "ip",
                "IP of will connect server",
                "IP",
                Occur::Optional,
                Some("127.0.0.1".to_string()));
    args.option("p",
                "port",
                "Port of will connect server",
                "PORT",
                Occur::Optional,
                Some(String::from("8080")));
    args.option("a",
                "Account",
                "Account for auto login",
                "ACC",
                Occur::Optional,
                None);
    args.option("s",
                "password",
                "Password for auto login",
                "PWD",
                Occur::Optional,
                None);

    args.parse(input)?;

    let help = args.value_of("help")?;
    if help {
        println!("{}",args.full_usage());
        return Err(ArgsError::new("","show help"));
    }
    let mut ip = Ipv4Addr::new(127,0,0,1);
    let mut port = 8080u16;
    let mut acc = None;
    let mut pwd = None;
    args.iter().for_each(|(k,v)|{
        match k.as_str() {
            "ip" => {
                match Ipv4Addr::from_str(v.as_str())
                {
                    Ok(i) => { ip = i;}
                    Err(_e) => { }
                }
            }
            "port" => {
                match u16::from_str(v.as_str())
                {
                    Ok(i) => { port = i;}
                    Err(_e) => { }
                }
            }
            "Account" => {
                acc = Some(v.clone());
            }
            "password" => {
                pwd = Some(v.clone());
            }
            _=>{}
        }
    });

    if acc.is_none() || pwd.is_none() {  acc = None; pwd = None;  }
    Ok(ClientArgs{
        ip,port,acc,pwd
    })
}