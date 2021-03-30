use quote::quote;

#[proc_macro]
pub fn gen_stream_parse(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ty:syn::Type = syn::parse(input).unwrap();

    let pm = quote! {
        impl StreamParse for #ty{
            fn stream_parse(stream:&mut Stream)->Option<Self>
            {
                match stream.next_range(size_of::<#ty>()){
                    Some(d) => {
                        let mut arr = [0;size_of::<#ty>()];
                        arr.copy_from_slice(d);
                        Some(#ty::from_be_bytes(arr))
                    }
                    None =>{ None }
                }
            }
        }
    };
    println!("{}", pm.to_string());
    pm.into()
}