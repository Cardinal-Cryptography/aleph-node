mod code_generation;
mod generation_utils;
mod intermediate_representation;
mod naming;
mod parse_utils;

use proc_macro::TokenStream;
use syn::{ItemMod, Result as SynResult};

use crate::{code_generation::generate_code, intermediate_representation::IR};

#[proc_macro_attribute]
pub fn snark_relation(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mod_ast = syn::parse_macro_input!(item as ItemMod);
    match _snark_relation(mod_ast) {
        Ok(token_stream) => token_stream,
        Err(e) => e.to_compile_error().into(),
    }
}

fn _snark_relation(mod_ast: ItemMod) -> SynResult<TokenStream> {
    let ir = IR::try_from(mod_ast)?;
    let code = generate_code(ir)?;
    Ok(code.into())
}
