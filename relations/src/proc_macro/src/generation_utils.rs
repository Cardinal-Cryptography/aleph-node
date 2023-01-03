use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{spanned::Spanned, Error as SynError, Result as SynResult};

use crate::intermediate_representation::{PublicInputField, RelationField};

fn get_ident(f: &RelationField) -> &Ident {
    f.field
        .ident
        .as_ref()
        .expect("We are working on named fields")
}

fn map_fields_with_ident<T, F: Into<RelationField> + Clone, M: Fn(&RelationField, &Ident) -> T>(
    fields: &[F],
    mapper: M,
) -> Vec<T> {
    fields
        .iter()
        .map(|f| {
            let f = f.clone().into();
            mapper(&f, get_ident(&f))
        })
        .collect()
}

pub(super) fn field_frontend_decls<F: Into<RelationField> + Clone>(
    fields: &[F],
) -> Vec<TokenStream2> {
    map_fields_with_ident(fields, |rf, ident| {
        let maybe_frontend_type = &rf.frontend_type;
        let backend_type = &rf.field.ty;

        maybe_frontend_type.as_ref().map_or_else(
            || quote! { #ident: #backend_type },
            |ft| {
                let ft = Ident::new(ft.as_str(), Span::call_site());
                quote! { #ident: #ft }
            },
        )
    })
}

pub(super) fn field_backend_decls<F: Into<RelationField> + Clone>(
    fields: &[F],
) -> Vec<TokenStream2> {
    map_fields_with_ident(fields, |rf, ident| {
        let ty = &rf.field.ty;
        quote! { #ident: #ty }
    })
}

pub(super) fn field_serializations(fields: &[PublicInputField], obj: &Ident) -> Vec<TokenStream2> {
    fields
        .iter()
        .map(|f| {
            let ident = f
                .inner
                .field
                .ident
                .as_ref()
                .expect("We are working on named fields");

            match &f.serialize_with {
                None => quote! { #obj . #ident },
                Some(serializer) => {
                    let serializer = Ident::new(serializer, Span::call_site());
                    quote! { #serializer ( & #obj . #ident ) }
                }
            }
        })
        .collect()
}

pub(super) fn field_rewrites<F: Into<RelationField> + Clone>(
    fields: &[F],
    obj: &Ident,
) -> Vec<TokenStream2> {
    map_fields_with_ident(fields, |_, ident| {
        quote! { #ident : #obj . #ident }
    })
}

pub(super) fn plain_field_getters<F: Into<RelationField> + Clone>(
    fields: &[F],
) -> Vec<TokenStream2> {
    map_fields_with_ident(fields, |rf, ident| {
        let backend_type = &rf.field.ty;
        quote! {
            pub fn #ident(&self) -> & #backend_type {
                &self . #ident
            }
        }
    })
}

pub(super) fn field_castings<F: Into<RelationField> + Clone>(
    fields: &[F],
) -> SynResult<Vec<TokenStream2>> {
    map_fields_with_ident(fields, |rf, ident| {
        let maybe_frontend_type = &rf.frontend_type;
        let maybe_parser = &rf.parse_with;

        match (maybe_frontend_type, maybe_parser) {
            (None, None) => Ok(quote! { #ident }),
            (None, Some(_)) => Err(SynError::new(
                rf.field.span(),
                "Parser is provided, but frontend type is absent.",
            )),
            (Some(_), None) => Ok(quote! { #ident : #ident . into() }),
            (Some(_), Some(parser)) => {
                let parser = Ident::new(parser, Span::call_site());
                Ok(quote! { #ident : #parser ( #ident ) })
            }
        }
    })
    .into_iter()
    .collect()
}

pub(super) fn successful_field_getters<F: Into<RelationField> + Clone>(
    fields: &[F],
) -> Vec<TokenStream2> {
    map_fields_with_ident(fields, |rf, ident| {
        let backend_type = &rf.field.ty;
        quote! {
            pub fn #ident(&self) -> Result<& #backend_type, ark_relations::r1cs::SynthesisError> {
                Ok(&self . #ident)
            }
        }
    })
}

pub(super) fn failing_field_getters<F: Into<RelationField> + Clone>(
    fields: &[F],
) -> Vec<TokenStream2> {
    map_fields_with_ident(fields, |rf, ident| {
        let backend_type = &rf.field.ty;
        quote! {
            pub fn #ident(&self) -> Result<& #backend_type, ark_relations::r1cs::SynthesisError> {
                Err(ark_relations::r1cs::SynthesisError::AssignmentMissing)
            }
        }
    })
}
