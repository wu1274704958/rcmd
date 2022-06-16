use quote::quote;
use syn::DeriveInput;
use regex::Regex;

#[proc_macro]
pub fn gen_cycle_num(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let in_ty = input.to_string();

    let in_:proc_macro2::TokenStream = in_ty.parse().unwrap();

    let result = quote! {
        impl CycleNum for #in_ {
            fn uncheck_add_assign(&mut self,v:Self)
            {
                let a = Wrapping(*self);
                let b = Wrapping(v);
                *self = (a + b).0;
            }
            fn in_min_range(&self,v:Self,r:Self) -> bool
            {
                v >= #in_::MIN && v <= #in_::MIN + r - 1
            }
            fn in_max_range(&self,v:Self,r:Self) -> bool
            {
                v >= #in_::MAX - r + 1 && v <= #in_::MAX
            }
            fn cycle_partial_cmp(&self,v:&Self,r:Self) -> Option<Ordering>
            {
                Some(self.cycle_cmp(v,r))
            }
            fn cycle_cmp(&self,v:&Self,r:Self) -> Ordering
            {
                if self.in_min_range(*self,r) && self.in_max_range(*v,r)  {
                    Ordering::Greater
                }else
                if self.in_max_range(*self,r) && self.in_min_range(*v,r) {
                    Ordering::Less
                }else {
                    self.cmp(v)
                }
            }
        }
    };
    //println!(" derive_stream_parse ---> \n{}",result.to_string());
    result.into()
}