use quote::quote;
use syn::DeriveInput;
use syn::LitInt;

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
    //println!("{}", pm.to_string());
    pm.into()
}

#[proc_macro_derive(StreamParse)]
pub fn derive_stream_parse(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast:DeriveInput = syn::parse(input).unwrap();

    let result = match ast.data {
        syn::Data::Struct(ref s) => {
            impl_stream_parse(&ast,&s.fields)
        },
        _ => panic!("doesn't work with unions yet"),
    };
    println!(" derive_stream_parse ---> \n{}",result.to_string());
    result.into()
}

fn impl_stream_parse(ast:&DeriveInput, field:&syn::Fields) -> proc_macro2::TokenStream
{
    let struct_name = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();
    let mut es:Vec<proc_macro2::TokenStream> = Vec::new();

    match field {
        syn::Fields::Named(ref fs) => {
            fs.named.iter().for_each(|it|{
                let name = if let Some(ref temp) = (*it).ident{
                    temp
                }else{
                    panic!("ident is None!");
                };
                es.push(quote!{ if !self.#name.stream_parse_ex(stream) {return false;}  } );
            });
        }
        syn::Fields::Unnamed(unname) => {
            for i in 0..unname.unnamed.len()
            {
                es.push(quote!{ if !self.#i.stream_parse_ex(stream) {return false;}  } );
            }
        }
        syn::Fields::Unit => {

        }
    }

    let es = quote!{  #(#es)* };
    quote!{
        impl #impl_generics StreamParse for #struct_name #ty_generics #where_clause {
            fn stream_parse(stream:&mut Stream)->Option<Self>{
                None
            }
            fn stream_parse_ex(&mut self, stream: &mut Stream) -> bool {
                #es
                true
            }
        }
    }
}