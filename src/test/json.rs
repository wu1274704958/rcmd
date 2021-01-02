use serde::{Serialize, Deserialize};

#[derive(Clone,Debug,PartialEq,Eq, Serialize, Deserialize)]
pub struct RegUser
{
    pub name:String,
    pub acc:String,
    pub pwd:String,
}


fn main()
{
    let o:serde_json::Value = serde_json::from_str(r#"{"a":"abc","b":90}"#).unwrap();

    dbg!(o);


    let u:RegUser = serde_json::from_str(r#"{"acc":"abc","name":"wws","pwd":"okkkkkk","unuse":67}"#).unwrap();
    dbg!(u);
}