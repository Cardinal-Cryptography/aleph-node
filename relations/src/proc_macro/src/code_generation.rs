use std::collections::HashSet;

use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{spanned::Spanned, Error as SynError, Result as SynResult};

use crate::{
    generation_utils::{
        failing_field_getters, field_backend_decls, field_castings, field_frontend_decls,
        field_rewrites, field_serializations, plain_field_getters, successful_field_getters,
    },
    intermediate_representation::{PublicInputField, IR},
    naming::{struct_name_with_full, struct_name_with_public, struct_name_without_input},
};

/// Generates the whole code based on the intermediate representation.
pub(super) fn generate_code(ir: IR) -> SynResult<TokenStream2> {
    let imports = &ir.imports;
    let circuit_field = &ir.circuit_field;

    let blocks = [
        quote! { #(#imports)* },
        quote! { #circuit_field },
        generate_relation_without_input(&ir)?,
        generate_relation_with_public(&ir)?,
        generate_relation_with_full(&ir)?,
        generate_circuit_definitions(&ir),
    ];

    Ok(TokenStream2::from_iter(blocks))
}

/// Generates struct, constructor and getters for the relation object with constants only.
fn generate_relation_without_input(ir: &IR) -> SynResult<TokenStream2> {
    let struct_name = struct_name_without_input(&ir.relation_base_name);
    let const_frontend_decls = field_frontend_decls(&ir.constants);
    let const_backend_decls = field_backend_decls(&ir.constants);
    let const_castings = field_castings(&ir.constants)?;
    let getters = [
        plain_field_getters(&ir.constants),
        failing_field_getters(&ir.public_inputs),
        failing_field_getters(&ir.private_inputs),
    ]
    .concat();

    Ok(quote! {
        pub struct #struct_name {
            #(#const_backend_decls),*
        }
        impl #struct_name {
            pub fn new(#(#const_frontend_decls),*) -> Self {
                Self { #(#const_castings),* }
            }
            #(#getters)*
        }
    })
}

/// Returns reordered copies of the elements in `fields`.
fn get_ordered_inputs(fields: &[PublicInputField]) -> SynResult<Vec<PublicInputField>> {
    #[derive(Copy, Clone)]
    enum Ordering {
        Unknown,
        Explicit,
        Implicit,
    }

    let mut ordering = Ordering::Unknown;
    let mut ordered = vec![];
    let mut orders = HashSet::new();

    for (idx, f) in fields.iter().enumerate() {
        let maybe_explicit_order = f.order;
        if matches!(ordering, Ordering::Unknown) {
            ordering = maybe_explicit_order.map_or(Ordering::Implicit, |_| Ordering::Explicit);
        }

        let order = match (maybe_explicit_order, ordering) {
            (None, Ordering::Explicit) | (Some(_), Ordering::Implicit) => {
                return Err(SynError::new(
                    f.inner.field.span(),
                    "Either all or none of the fields should be explicitly ordered",
                ))
            }
            (Some(order), Ordering::Explicit) => order,
            (None, Ordering::Implicit) => idx,
            _ => unreachable!(),
        };

        if orders.contains(&order) {
            return Err(SynError::new(f.inner.field.span(), "Orders must be unique"));
        }
        orders.insert(order);
        ordered.push((f.clone(), order));
    }

    ordered.sort_by_key(|(_, o)| *o);
    Ok(ordered.into_iter().map(|(f, _)| f).collect())
}

fn generate_public_input_serialization(ir: &IR) -> SynResult<TokenStream2> {
    let circuit_field = &ir.circuit_field.ident;
    let inputs = get_ordered_inputs(&ir.public_inputs)?;
    let accesses = field_serializations(&inputs, &Ident::new("self", Span::call_site()));

    Ok(quote! {
        pub fn serialize_public_input(&self) -> ark_std::vec::Vec<#circuit_field> {
            [ #(#accesses),* ].concat()
        }
    })
}

/// Generates struct, constructor, getters, public input serialization and downcasting for the
/// relation object with constants and public input.
fn generate_relation_with_public(ir: &IR) -> SynResult<TokenStream2> {
    let struct_name = struct_name_with_public(&ir.relation_base_name);
    let struct_name_without_input = struct_name_without_input(&ir.relation_base_name);
    let object_ident = Ident::new("obj", Span::call_site());

    let backend_decls = [
        field_backend_decls(&ir.constants),
        field_backend_decls(&ir.public_inputs),
    ]
    .concat();
    let frontend_decls = [
        field_frontend_decls(&ir.constants),
        field_frontend_decls(&ir.public_inputs),
    ]
    .concat();
    let castings = [
        field_castings(&ir.constants)?,
        field_castings(&ir.public_inputs)?,
    ]
    .concat();
    let getters = [
        plain_field_getters(&ir.constants),
        successful_field_getters(&ir.public_inputs),
        failing_field_getters(&ir.private_inputs),
    ]
    .concat();

    let const_rewrites = field_rewrites(&ir.constants, &object_ident);

    let public_input_serialization = generate_public_input_serialization(ir)?;

    Ok(quote! {
        pub struct #struct_name {
            #(#backend_decls),*
        }
        impl #struct_name {
            pub fn new(#(#frontend_decls),*) -> Self {
                Self { #(#castings),* }
            }

            #(#getters)*

            #public_input_serialization
        }

        impl From<#struct_name> for #struct_name_without_input {
            fn from(#object_ident: #struct_name) -> Self {
                Self { #(#const_rewrites),* }
            }
        }
    })
}

/// Generates struct, constructor, getters downcasting for the full relation object.
fn generate_relation_with_full(ir: &IR) -> SynResult<TokenStream2> {
    let struct_name = struct_name_with_full(&ir.relation_base_name);
    let struct_name_with_public = struct_name_with_public(&ir.relation_base_name);
    let object_ident = Ident::new("obj", Span::call_site());

    let backend_decls = [
        field_backend_decls(&ir.constants),
        field_backend_decls(&ir.public_inputs),
        field_backend_decls(&ir.private_inputs),
    ]
    .concat();
    let frontend_decls = [
        field_frontend_decls(&ir.constants),
        field_frontend_decls(&ir.public_inputs),
        field_frontend_decls(&ir.private_inputs),
    ]
    .concat();
    let castings = [
        field_castings(&ir.constants)?,
        field_castings(&ir.public_inputs)?,
        field_castings(&ir.private_inputs)?,
    ]
    .concat();

    let getters = [
        plain_field_getters(&ir.constants),
        successful_field_getters(&ir.public_inputs),
        successful_field_getters(&ir.private_inputs),
    ]
    .concat();

    let const_and_public_rewrites = [
        field_rewrites(&ir.constants, &object_ident),
        field_rewrites(&ir.public_inputs, &object_ident),
    ]
    .concat();

    Ok(quote! {
        pub struct #struct_name {
            #(#backend_decls),*
        }
        impl #struct_name {
            pub fn new(#(#frontend_decls),*) -> Self {
                Self { #(#castings),* }
            }

            #(#getters)*
        }

        impl From<#struct_name> for #struct_name_with_public {
            fn from(#object_ident: #struct_name) -> Self {
                Self { #(#const_and_public_rewrites),* }
            }
        }
    })
}

/// Generates `ConstraintSynthesizer` implementations.
fn generate_circuit_definitions(ir: &IR) -> TokenStream2 {
    let struct_name_without_input = struct_name_without_input(&ir.relation_base_name);
    let struct_name_with_full = struct_name_with_full(&ir.relation_base_name);

    let body = &ir.circuit_definition.block.stmts;
    let cf = &ir.circuit_field.ident;

    quote! {
        impl ark_relations::r1cs::ConstraintSynthesizer<#cf> for #struct_name_without_input {
            fn generate_constraints(
                self,
                cs: ark_relations::r1cs::ConstraintSystemRef<#cf>
            ) -> ark_relations::r1cs::Result<()> {
                if cs.is_in_setup_mode() {
                    #(#body)*
                } else {
                    #[cfg(feature = "std")] {
                        eprintln!("For proof generation, you should use relation object with full input.");
                    }
                    Err(ark_relations::r1cs::SynthesisError::AssignmentMissing)
                }
            }
        }

        impl ark_relations::r1cs::ConstraintSynthesizer<#cf> for #struct_name_with_full {
            fn generate_constraints(
                self,
                cs: ark_relations::r1cs::ConstraintSystemRef<#cf>
            ) -> ark_relations::r1cs::Result<()> {
                    #(#body)*
            }
        }
    }
}
