use darling::FromField;
use proc_macro2::TokenStream;
use proc_macro_error::abort;
use quote::quote;
use syn::{
    parse_quote,
    Data,
    DataStruct,
    DeriveInput,
};

use crate::{
    error::Error,
    options::{
        Bitfield,
        DeriveOptions,
        FieldOptions,
    },
    util::{
        field_name,
        make_where_clause,
    },
};

pub fn derive_write(item: DeriveInput, options: DeriveOptions) -> Result<TokenStream, Error> {
    let ident = &item.ident;
    if let Some(bitfield) = &options.bitfield {
        match &item.data {
            Data::Struct(s) => derive_write_for_struct_bitfield(s, bitfield, &item, &options),
            _ => abort!(ident, "Bitfields can only be derived on structs."),
        }
    }
    else {
        match &item.data {
            Data::Struct(s) => derive_write_for_struct(&s, &item, &options),
            Data::Enum(_) => todo!(),
            Data::Union(_) => abort!(ident, "Write can't be derive on unions."),
        }
    }
}

fn derive_write_for_struct(
    s: &DataStruct,
    item: &DeriveInput,
    _options: &DeriveOptions,
) -> Result<TokenStream, Error> {
    let ident = &item.ident;
    let (impl_generics, type_generics, where_clause) = item.generics.split_for_impl();
    let mut where_clause = make_where_clause(where_clause);

    let mut write_fields = Vec::with_capacity(s.fields.len());

    for (i, field) in s.fields.iter().enumerate() {
        let (_, field_name) = field_name(i, field);
        let field_options = FieldOptions::from_field(&field)?;
        let field_ty = &field.ty;

        if let Some(endianness) = field_options.endianness.ty() {
            write_fields.push(quote! {
                ::skunk::util::bytes::rw::WriteXe::<_, #endianness>::write(&self.#field_name, &mut writer)?;
            });
            where_clause.predicates.push(parse_quote! { #field_ty: for<'w> ::skunk::util::bytes::rw::WriteXe::<&'w mut __W, #endianness> });
        }
        else {
            write_fields.push(quote! {
                ::skunk::util::bytes::rw::Write::<_>::write(&self.#field_name, &mut writer)?;
            });
            where_clause.predicates.push(
                parse_quote! { #field_ty: for<'w> ::skunk::util::bytes::rw::Write::<&'w mut __W> },
            );
        }
    }

    Ok(quote! {
        #[automatically_derived]
        impl<__W, #impl_generics> ::skunk::util::bytes::rw::Write<__W> for #ident<#type_generics> #where_clause {
            fn write(&self, mut writer: __W) -> ::std::result::Result<(), ::skunk::util::bytes::rw::Full> {
                #(#write_fields)*
                ::std::result::Result::Ok(())
            }
        }
    })
}

fn derive_write_for_struct_bitfield(
    _s: &DataStruct,
    _bitfield: &Bitfield,
    _item: &DeriveInput,
    _options: &DeriveOptions,
) -> Result<TokenStream, Error> {
    todo!();
}