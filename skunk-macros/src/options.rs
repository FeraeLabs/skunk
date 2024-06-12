use darling::{
    FromDeriveInput,
    FromField,
    FromMeta,
};
use proc_macro2::TokenStream;
use proc_macro_error::abort_call_site;
use quote::quote;
use syn::Path;

#[derive(FromDeriveInput)]
#[darling(attributes(skunk), forward_attrs(allow, doc, cfg))]
pub struct DeriveOptions {
    pub bitfield: Option<Bitfield>,
}

#[derive(FromField)]
#[darling(attributes(skunk))]
pub struct FieldOptions {
    #[darling(flatten)]
    pub endianness: Endianness,
}

#[derive(FromMeta)]
pub struct Bitfield {
    pub ty: Path,

    #[darling(flatten)]
    pub endianness: Endianness,
}

#[derive(FromField)]
#[darling(attributes(skunk))]
pub struct BitfieldFieldOptions {
    pub bits: Option<usize>,
    pub start: Option<usize>,
    pub end: Option<usize>,
}

#[derive(FromMeta)]
pub struct Endianness {
    pub endianness: Option<Path>,

    #[darling(default)]
    pub big: bool,

    #[darling(default)]
    pub little: bool,

    #[darling(default)]
    pub network: bool,

    #[darling(default)]
    pub native: bool,
}

impl Endianness {
    pub fn ty(&self) -> Option<TokenStream> {
        match (
            self.big,
            self.little,
            self.network,
            self.native,
            &self.endianness,
        ) {
            (false, false, false, false, None) => None,
            (true, false, false, false, None) => {
                Some(quote! { ::skunk::util::bytes::endianness::BigEndian })
            }
            (false, true, false, false, None) => {
                Some(quote! { ::skunk::util::bytes::endianness::LittleEndian })
            }
            (false, false, true, false, None) => {
                Some(quote! { ::skunk::util::bytes::endianness::NetworkEndian })
            }
            (false, false, false, true, None) => {
                Some(quote! { ::skunk::util::bytes::endianness::NativeEndian })
            }
            (false, false, false, false, Some(path)) => Some(quote! { #path }),
            _ => {
                abort_call_site!(
                    "Only one of `big`, `little`, `network`, `native`, or `endianness = PATH` may be specified."
                )
            }
        }
    }
}
