use std::mem::size_of;
use pm_gen::gen_stream_parse;
use pm_gen::StreamParse;

#[derive(Debug)]
pub struct Stream<'a>{
    data:&'a [u8],
    ptr: usize
}

impl<'a> Stream<'a>{
    pub fn new(data:&'a[u8])->Stream<'a>
    {
        Stream{
            data,
            ptr : 0
        }
    }

    pub fn next(&mut self)-> Option<u8>
    {
        if self.ptr >= self.data.len() { return None;}
        let p = self.ptr;
        self.ptr += 1;
        Some(self.data[p])
    }

    pub fn next_range(&mut self,len:usize) -> Option<&'a[u8]>
    {
        let e = self.ptr + (len - 1);
        if e >= self.data.len() { return None;}
        let p = self.ptr;
        self.ptr += len;
        Some(&self.data[p..=e])
    }

    pub fn skip(&mut self,n:usize) -> bool
    {
        let e = self.ptr + (n - 1);
        if e >= self.data.len() { return false;}
        self.ptr += n;
        true
    }

    pub fn get_rest(&mut self) -> &'a[u8]
    {
        let res = &self.data[self.ptr..];
        self.ptr += res.len();
        res
    }

}

pub trait StreamParse : Sized {
    fn stream_parse(stream:&mut Stream)->Option<Self>;
    fn stream_parse_ex(&mut self,stream:&mut Stream)->bool
    {
        if let Some(v) = Self::stream_parse(stream){
            *self = v;
            true
        }else {
            false
        }
    }
}

pub struct Skip<const N:usize>{

}

impl<const N:usize> StreamParse for Skip<N> {
    fn stream_parse(stream: &mut Stream) -> Option<Self>
    {
        if stream.skip(N) { Some(Skip::<N>{}) }else { None }
    }
}

pub struct SkipRt{
    n:usize
}

impl From<usize> for SkipRt{
    fn from(v: usize) -> Self
    {
        SkipRt{n:v}
    }
}

impl Into<usize> for SkipRt{
    fn into(self) -> usize {
        self.n
    }
}

impl StreamParse for SkipRt {
    fn stream_parse(_stream: &mut Stream) -> Option<Self> {
        None
    }

    fn stream_parse_ex(&mut self, stream: &mut Stream) -> bool {
        stream.skip(self.n)
    }
}

gen_stream_parse!{f32}
gen_stream_parse!{f64}
gen_stream_parse!{u64}
gen_stream_parse!{i64}
gen_stream_parse!{u16}
gen_stream_parse!{i16}
gen_stream_parse!{u32}
gen_stream_parse!{i32}
gen_stream_parse!{usize}
gen_stream_parse!{isize}
gen_stream_parse!{u128}
gen_stream_parse!{i128}

#[test]
fn test_gen_stream_parse2() {
    let a1 = 10052398u32;
    let a2 = -1293029103i32;
    let a3 = 129039102930193322usize;
    let a4 = -1290391030190130192isize;

    let mut v = Vec::new();
    v.extend_from_slice(&a1.to_be_bytes());
    v.extend_from_slice(&a2.to_be_bytes());
    v.push(1);
    v.push(2);
    v.push(3);
    v.extend_from_slice(&a3.to_be_bytes());
    v.extend_from_slice(&a4.to_be_bytes());

    let buf = v.as_slice();
    let mut stream = Stream::new(buf);
    let b1 = u32::stream_parse(&mut stream).unwrap();
    let b2 = i32::stream_parse(&mut stream).unwrap();
    SkipRt{n:3}.stream_parse_ex(&mut stream);
    let b3 = usize::stream_parse(&mut stream).unwrap();
    let b4 = isize::stream_parse(&mut stream).unwrap();

    assert_eq!(a1,b1);
    assert_eq!(a2,b2);
    assert_eq!(a3,b3);
    assert_eq!(a4,b4);
}

#[test]
fn test_gen_stream_parse() {
    let a = 10052398u32;
    let x = 9090909i32;
    let y = 871923i32;
    let z = 2787183789127usize;
    let mut v = Vec::new();
    v.extend_from_slice(&a.to_be_bytes());
    v.extend_from_slice(&x.to_be_bytes());
    v.extend_from_slice(&y.to_be_bytes());
    v.extend_from_slice(&z.to_be_bytes());
    let mut stream = Stream::new(&v);
    let b = u32::stream_parse(&mut stream).unwrap();
    assert_eq!(a,b);

    let mut data = Data{ x:0,y:0,z:0};
    data.stream_parse_ex(&mut stream);

    assert_eq!(x,data.x);
    assert_eq!(y,data.y);
    assert_eq!(z,data.z);

    let mut v2 = Vec::new();
    v2.extend_from_slice(&x.to_be_bytes());
    v2.extend_from_slice(&y.to_be_bytes());
    v2.push(2);
    v2.push(3);
    v2.extend_from_slice(&z.to_be_bytes());
    let mut stream = Stream::new(&v2);

    let mut data2 = MyData(1,2,Skip::<2>{},3);
    data2.stream_parse_ex(&mut stream);

    assert_eq!(x,data2.0);
    assert_eq!(y,data2.1);
    assert_eq!(z,data2.3);
    {
        let a = [0,1,2,3,4];
        let mut stream = Stream::new(a.as_ref());
        stream.next();
        stream.next();
        assert_eq!( &[2,3,4],stream.get_rest());
        assert_eq!( &[] as &[u8],stream.get_rest());
    }
}

#[derive(StreamParse)]
struct Data{
    pub x: i32,
    pub y: i32,
    pub z: usize
}

#[derive(StreamParse)]
struct MyData(i32,i32,Skip<2>,usize);
