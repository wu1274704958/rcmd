fn main()
{
    let o:serde_json::Value = serde_json::from_str(r#"{"a":"abc","b":90}"#).unwrap();

    dbg!(o);
}