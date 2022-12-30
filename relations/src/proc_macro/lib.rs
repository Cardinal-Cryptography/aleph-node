use proc_macro::{Span, TokenStream};
use proc_macro2::{Ident, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{
    spanned::Spanned, Error as SynError, Field, Fields, FieldsNamed, Item, ItemFn, ItemMod,
    ItemStruct, Result as SynResult, Visibility,
};

mod keyword {
    pub const RELATION_OBJECT_DEF: &str = "relation_object_definition";
    pub const CIRCUIT_DEF: &str = "circuit_definition";

    pub const CONSTANT_FIELD: &str = "constant";
    pub const PUBLIC_INPUT_FIELD: &str = "public_input";
    pub const PRIVATE_INPUT_FIELD: &str = "private_input";
}

#[proc_macro_attribute]
pub fn snark_relation(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mod_ast = syn::parse_macro_input!(item as ItemMod);
    match process(mod_ast) {
        Ok(token_stream) => token_stream,
        Err(e) => return e.to_compile_error().into(),
    }
}

fn process(mod_ast: ItemMod) -> SynResult<TokenStream> {
    let (relation_ast, circuit_def_ast) = analyze_module(&mod_ast)?;

    let relation_base_name = relation_ast.ident;

    // Warn about items visibility.
    if !matches!(relation_ast.vis, Visibility::Inherited) {
        eprintln!("Warning: The `{relation_base_name}` struct is public, but will be erased.")
    };
    if !matches!(circuit_def_ast.vis, Visibility::Inherited) {
        eprintln!("Warning: The circuit definition is public, but will be erased.")
    }

    // Extract all fields. There should be at least one field. All fields must be named.
    let fields = match relation_ast.fields {
        Fields::Named(fields) => Ok(fields),
        _ => Err(SynError::new(
            relation_ast.fields.span(),
            "The relation should have some fields and they should be named.",
        )),
    }?;

    // Segregate fields.
    let constants = extract_relation_fields(&fields, keyword::CONSTANT_FIELD)?;
    let public_inputs = extract_relation_fields(&fields, keyword::PUBLIC_INPUT_FIELD)?;
    let private_inputs = extract_relation_fields(&fields, keyword::PRIVATE_INPUT_FIELD)?;

    // Generate structs, castings and constructors
    let code = generate_code(
        &relation_base_name,
        &constants,
        &public_inputs,
        &private_inputs,
    );

    Ok(code)
}

fn is_circuit_def(item_fn: &ItemFn) -> bool {
    item_fn
        .attrs
        .iter()
        .any(|a| a.path.is_ident(keyword::CIRCUIT_DEF))
}

fn is_relation_object_def(item_struct: &ItemStruct) -> bool {
    item_struct
        .attrs
        .iter()
        .any(|a| a.path.is_ident(keyword::RELATION_OBJECT_DEF))
}

fn analyze_module(mod_ast: &ItemMod) -> SynResult<(ItemStruct, ItemFn)> {
    let items = &mod_ast
        .content
        .as_ref()
        .ok_or_else(|| {
            SynError::new(
                mod_ast.span(),
                "Invalid module - it is expected to be inlined",
            )
        })?
        .1;

    let mut struct_ast = None;
    let mut circuit_ast = None;

    for item in items {
        match item {
            Item::Fn(item_fn) if is_circuit_def(item_fn) => {
                if let Some(_) = circuit_ast {
                    return Err(SynError::new(
                        item_fn.span(),
                        "Circuit defined for the second time",
                    ));
                }
                circuit_ast = Some(item_fn.clone());
            }
            Item::Struct(item_struct) if is_relation_object_def(item_struct) => {
                if let Some(_) = struct_ast {
                    return Err(SynError::new(
                        item_struct.span(),
                        "Relation object defined for the second time",
                    ));
                }
                struct_ast = Some(item_struct.clone());
            }
            _ => {}
        }
    }

    Ok((
        struct_ast.ok_or_else(|| SynError::new(mod_ast.span(), "Missing relation definition"))?,
        circuit_ast.ok_or_else(|| SynError::new(mod_ast.span(), "Missing circuit definition"))?,
    ))
}

fn extract_relation_fields<FieldType: ?Sized>(
    fields: &FieldsNamed,
    field_type: &FieldType,
) -> SynResult<Vec<Field>>
where
    Ident: PartialEq<FieldType>,
{
    Ok(fields
        .named
        .iter()
        .filter(|f| f.attrs.iter().any(|a| a.path.is_ident(field_type)))
        .cloned()
        .collect())
}

fn field_decls(fields: &[Field]) -> Vec<TokenStream2> {
    fields
        .iter()
        .map(|Field { ident, ty, .. }| {
            let ident = ident.as_ref().expect("We are working on named fields");
            quote! { #ident: #ty }
        })
        .collect()
}

fn field_names(fields: &[Field]) -> Vec<TokenStream2> {
    fields
        .iter()
        .map(|f| {
            let ident = f.ident.as_ref().expect("We are working on named fields");
            quote! { #ident }
        })
        .collect()
}

fn field_accesses(fields: &[Field], obj: &Ident) -> Vec<TokenStream2> {
    fields
        .iter()
        .map(|f| {
            let ident = f.ident.as_ref().expect("We are working on named fields");
            quote! { #obj . #ident }
        })
        .collect()
}

fn generate_code(
    relation_base_name: &Ident,
    constants: &[Field],
    public_inputs: &[Field],
    private_inputs: &[Field],
) -> TokenStream {
    let struct_name_without_input = format_ident!("{relation_base_name}WithoutInput",);
    let struct_name_with_public = format_ident!("{relation_base_name}WithPublicInput",);
    let struct_name_with_full = format_ident!("{relation_base_name}WithFullInput",);

    let const_decls = field_decls(constants);
    let public_input_decls = field_decls(public_inputs);
    let private_input_decls = field_decls(private_inputs);

    let const_and_public_decls = [const_decls.clone(), public_input_decls.clone()].concat();
    let all_decls = [const_and_public_decls.clone(), private_input_decls.clone()].concat();

    let const_names = field_names(constants);
    let public_input_names = field_names(public_inputs);
    let private_input_names = field_names(private_inputs);

    let const_and_public_names = [const_names.clone(), public_input_names.clone()].concat();
    let all_names = [const_and_public_names.clone(), private_input_names.clone()].concat();

    let object_ident = Ident::new("obj", proc_macro2::Span::call_site());

    let const_accesses = field_accesses(constants, &object_ident);
    let public_input_accesses = field_accesses(public_inputs, &object_ident);

    let const_and_public_accesses =
        [const_accesses.clone(), public_input_accesses.clone()].concat();

    (quote! {
        pub struct #struct_name_without_input {
            #(#const_decls),*
        }
        impl #struct_name_without_input {
            pub fn new(#(#const_decls),*) -> Self {
                Self { #(#const_names),* }
            }
        }

        pub struct #struct_name_with_public {
            #(#const_and_public_decls),*
        }
        impl #struct_name_with_public {
            pub fn new(#(#const_and_public_decls),*) -> Self {
                Self { #(#const_and_public_names),* }
            }
        }

        pub struct #struct_name_with_full {
            #(#all_decls),*
        }
        impl #struct_name_with_full {
            pub fn new(#(#all_decls),*) -> Self {
                Self { #(#all_names),* }
            }
        }

        impl From<#struct_name_with_full> for #struct_name_with_public {
            fn from(#object_ident: #struct_name_with_full) -> Self {
                Self::new( #(#const_and_public_accesses),* )
            }
        }

        impl From<#struct_name_with_public> for #struct_name_without_input {
            fn from(#object_ident: #struct_name_with_public) -> Self {
                Self::new( #(#const_accesses),* )
            }
        }
    })
    .into()
}
