use std::str::FromStr;

use proc_macro2::{Ident, Span};
use syn::{
    spanned::Spanned, Error as SynError, Field, Fields, FieldsNamed, Item, ItemFn, ItemMod,
    ItemStruct, ItemType, ItemUse, Result as SynResult, VisPublic, Visibility,
};

use crate::{
    naming::{
        CIRCUIT_FIELD_DEF, CONSTANT_FIELD, FIELD_FRONTEND_TYPE, FIELD_PARSER, FIELD_SERIALIZER,
        PRIVATE_INPUT_FIELD, PUBLIC_INPUT_FIELD, PUBLIC_INPUT_ORDER,
    },
    parse_utils::{
        as_circuit_def, as_circuit_field_def, as_relation_object_def, get_field_attr,
        get_public_input_field_config, get_relation_field_config,
    },
};

#[derive(Clone)]
pub(super) struct RelationField {
    pub field: Field,
    pub frontend_type: Option<String>,
    pub parse_with: Option<String>,
}

impl TryFrom<Field> for RelationField {
    type Error = SynError;

    fn try_from(field: Field) -> Result<Self, Self::Error> {
        let attr = get_field_attr(&field)?;
        let config = get_relation_field_config(attr)?;

        Ok(RelationField {
            field,
            frontend_type: config.get(FIELD_FRONTEND_TYPE).cloned(),
            parse_with: config.get(FIELD_PARSER).cloned(),
        })
    }
}

#[derive(Clone)]
pub(super) struct PublicInputField {
    pub inner: RelationField,
    pub serialize_with: Option<String>,
    pub order: Option<usize>,
}

impl From<PublicInputField> for RelationField {
    fn from(public_input_field: PublicInputField) -> Self {
        public_input_field.inner
    }
}

impl TryFrom<Field> for PublicInputField {
    type Error = SynError;

    fn try_from(field: Field) -> Result<Self, Self::Error> {
        let attr = get_field_attr(&field)?;
        let config = get_public_input_field_config(attr)?;

        let order = match config.get(PUBLIC_INPUT_ORDER) {
            None => None,
            Some(s) => match usize::from_str(s) {
                Ok(order) => Some(order),
                Err(e) => {
                    return Err(SynError::new(
                        attr.span(),
                        format!("Invalid order value: {e:?}"),
                    ))
                }
            },
        };

        Ok(PublicInputField {
            inner: RelationField {
                field,
                frontend_type: config.get(FIELD_FRONTEND_TYPE).cloned(),
                parse_with: config.get(FIELD_PARSER).cloned(),
            },
            serialize_with: config.get(FIELD_SERIALIZER).cloned(),
            order,
        })
    }
}

pub(super) struct IR {
    pub relation_base_name: Ident,

    pub constants: Vec<RelationField>,
    pub public_inputs: Vec<PublicInputField>,
    pub private_inputs: Vec<RelationField>,

    pub circuit_field: ItemType,

    pub circuit_definition: ItemFn,

    pub imports: Vec<ItemUse>,
}

struct Items {
    struct_def: ItemStruct,
    circuit_def: ItemFn,
    circuit_field: ItemType,
    imports: Vec<ItemUse>,
}

impl TryFrom<ItemMod> for IR {
    type Error = SynError;

    fn try_from(item_mod: ItemMod) -> SynResult<Self> {
        let Items {
            struct_def,
            circuit_def: circuit_definition,
            mut circuit_field,
            imports,
        } = extract_items(item_mod)?;

        let relation_base_name = struct_def.ident.clone();

        // Warn about items visibility.
        if !matches!(struct_def.vis, Visibility::Inherited) {
            eprintln!("Warning: The `{relation_base_name}` struct is public, but will be erased.")
        };
        if !matches!(circuit_definition.vis, Visibility::Inherited) {
            eprintln!("Warning: The circuit definition is public, but will be erased.")
        }
        if !matches!(circuit_field.vis, Visibility::Public(_)) {
            eprintln!("Warning: The circuit field must be public. Visibility will be changed.");
            circuit_field.vis = Visibility::Public(VisPublic {
                pub_token: Default::default(),
            });
        }

        circuit_field
            .attrs
            .retain(|a| !a.path.is_ident(CIRCUIT_FIELD_DEF));

        // Extract all fields. There should be at least one field. All fields must be named.
        let fields = match struct_def.fields {
            Fields::Named(fields) => Ok(fields),
            _ => Err(SynError::new(
                struct_def.fields.span(),
                "The relation should have some fields and they should be named.",
            )),
        }?;

        // Segregate fields.
        let constants = extract_relation_fields(&fields, CONSTANT_FIELD)?;
        let public_inputs = extract_relation_fields(&fields, PUBLIC_INPUT_FIELD)?;
        let private_inputs = extract_relation_fields(&fields, PRIVATE_INPUT_FIELD)?;

        let constants = cast_fields(constants)?;
        let public_inputs = cast_fields(public_inputs)?;
        let private_inputs = cast_fields(private_inputs)?;

        Ok(IR {
            relation_base_name,
            constants,
            public_inputs,
            private_inputs,
            circuit_field,
            circuit_definition,
            imports,
        })
    }
}

fn extract_item<I: Spanned + Clone, E: Fn(&Item) -> Option<I>>(
    items: &[Item],
    extractor: E,
    outer_span: Span,
    item_name: &'static str,
) -> SynResult<I> {
    let matching = items.iter().filter_map(extractor).collect::<Vec<_>>();
    match matching.len() {
        0 => Err(SynError::new(
            outer_span,
            format!("Missing item: {item_name}"),
        )),
        1 => Ok(matching[0].clone()),
        _ => Err(SynError::new(
            matching[1].span(),
            format!("Duplicated item: {item_name}"),
        )),
    }
}

fn extract_items(item_mod: ItemMod) -> SynResult<Items> {
    let items = &item_mod
        .content
        .as_ref()
        .ok_or_else(|| {
            SynError::new(
                item_mod.span(),
                "Invalid module - it is expected to be inlined",
            )
        })?
        .1;

    let span = item_mod.span();

    let struct_def = extract_item(items, as_relation_object_def, span, "relation object")?;
    let circuit_def = extract_item(items, as_circuit_def, span, "circuit definition")?;
    let circuit_field = extract_item(items, as_circuit_field_def, span, "circuit field")?;

    let imports = items
        .iter()
        .filter_map(|i| match i {
            Item::Use(item_use) => Some(item_use.clone()),
            _ => None,
        })
        .collect();

    Ok(Items {
        struct_def,
        circuit_def,
        circuit_field,
        imports,
    })
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

fn cast_fields<F: TryFrom<Field, Error = SynError>>(fields: Vec<Field>) -> SynResult<Vec<F>> {
    fields
        .into_iter()
        .map(TryInto::<F>::try_into)
        .collect::<Vec<_>>()
        .into_iter()
        .collect()
}
