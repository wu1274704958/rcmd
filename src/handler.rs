pub trait Handle {
    fn handle(&self,data:&Vec<u8>)-> Vec<u8>;
}

#[derive(Copy, Clone)]
pub struct TestHandler{

}

impl Handle for TestHandler {
    fn handle(&self, _data: &Vec<u8>) -> Vec<u8> {
        return vec![7,b'{',b'"',b'r',b'e',b't',b'"',b':',b'0',b'}',9];
    }
}

