use darling::FromMeta;
use proc_macro::TokenStream;
use syn::{parse_macro_input, AttributeArgs, ItemStruct};

#[derive(Debug, FromMeta)]
struct MacroArgs {
    #[darling(default)]
    timeout_ms: Option<u16>,
    path: String,
}

#[proc_macro_attribute]
pub fn class(args: TokenStream, input: TokenStream) -> TokenStream {
    let attr_args = parse_macro_input!(args as AttributeArgs);
    let _input = parse_macro_input!(input as ItemStruct);

    let _args = match MacroArgs::from_list(&attr_args) {
        Ok(v) => v,
        Err(e) => {
            return TokenStream::from(e.write_errors());
        }
    };

    // do things with `args`
    unimplemented!()
}
