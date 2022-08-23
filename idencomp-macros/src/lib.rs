use proc_macro::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{parenthesized, parse_macro_input, Ident, Lit, Token};

enum ModelItem {
    Dummy,
    Generic {
        acids: Lit,
        q_scores: Lit,
        position_bits: Lit,
    },
    Light {
        acids: Lit,
        q_scores: Lit,
        position_bits: Lit,
        q_score_max: Lit,
    },
}

impl ModelItem {
    pub fn as_type_enum_variant(&self) -> proc_macro2::TokenStream {
        let enum_ident = self.enum_identifier();
        let serde_ident = self.serde_identifier();
        let doc = self.as_enum_variant_doc();

        quote! {
            #[doc = #doc]
            #[serde(rename = #serde_ident)]
            #enum_ident
        }
    }

    pub fn as_enum_value(&self) -> proc_macro2::TokenStream {
        let enum_ident = self.enum_identifier();

        quote! {
            ContextSpecType::#enum_ident
        }
    }

    pub fn as_name_variant(&self) -> proc_macro2::TokenStream {
        let enum_ident = self.enum_identifier();
        let serde_ident = self.serde_identifier();

        quote! {
            ContextSpecType::#enum_ident => #serde_ident
        }
    }

    pub fn as_generator_variant(&self) -> proc_macro2::TokenStream {
        let enum_ident = self.enum_identifier();
        let constructor = self.as_generator_constructor();

        quote! {
            ContextSpecType::#enum_ident => {
                Box::new(#constructor)
            }
        }
    }

    fn as_generator_constructor(&self) -> proc_macro2::TokenStream {
        let generator_type = self.as_generator_type();

        quote! {
            #generator_type::new(length)
        }
    }

    fn as_spec_num_variant(&self) -> proc_macro2::TokenStream {
        let enum_ident = self.enum_identifier();
        let spec_num = self.as_spec_num();

        quote! {
            ContextSpecType::#enum_ident => {
                #spec_num
            }
        }
    }

    fn as_spec_num(&self) -> proc_macro2::TokenStream {
        let generator_type = self.as_generator_type();

        quote! {
            #generator_type::spec_num()
        }
    }

    fn as_generator_type(&self) -> proc_macro2::TokenStream {
        match self {
            ModelItem::Dummy => quote! {
                GenericContextSpecGenerator::<0, 0, 0>
            },
            ModelItem::Generic {
                acids,
                q_scores,
                position_bits,
            } => quote! {
                GenericContextSpecGenerator::<#acids, #q_scores, #position_bits>
            },
            ModelItem::Light {
                acids,
                q_scores,
                position_bits,
                q_score_max,
            } => quote! {
                LightContextSpecGenerator::<#acids, #q_scores, #position_bits, #q_score_max>
            },
        }
    }

    fn as_enum_variant_doc(&self) -> String {
        match self {
            ModelItem::Dummy => "Dummy context (i.e. no context information).".to_owned(),
            ModelItem::Generic { acids, q_scores, position_bits } => format!(
                "Generic context that includes {} prior acids, {} quality scores, and {} position bits.",
                acids.to_token_stream(),
                q_scores.to_token_stream(),
                position_bits.to_token_stream(),
            ),
            ModelItem::Light { acids, q_scores, position_bits, q_score_max } => format!(
                "Light context that includes {} prior acids, {} quality scores (max {}), and {} position bits.",
                acids.to_token_stream(),
                q_scores.to_token_stream(),
                q_score_max.to_token_stream(),
                position_bits.to_token_stream(),
            ),
        }
    }

    fn enum_identifier(&self) -> Ident {
        match self {
            ModelItem::Dummy => {
                format_ident!("Dummy")
            }
            ModelItem::Generic {
                acids,
                q_scores,
                position_bits,
            } => {
                format_ident!(
                    "Generic{}Acids{}QScores{}PosBits",
                    acids.to_token_stream().to_string(),
                    q_scores.to_token_stream().to_string(),
                    position_bits.to_token_stream().to_string(),
                )
            }
            ModelItem::Light {
                acids,
                q_scores,
                position_bits,
                q_score_max,
            } => {
                format_ident!(
                    "Light{}Acids{}QScores{}PosBits{}MaxQScore",
                    acids.to_token_stream().to_string(),
                    q_scores.to_token_stream().to_string(),
                    position_bits.to_token_stream().to_string(),
                    q_score_max.to_token_stream().to_string(),
                )
            }
        }
    }

    fn serde_identifier(&self) -> String {
        match self {
            ModelItem::Dummy => "dummy".to_string(),
            ModelItem::Generic {
                acids,
                q_scores,
                position_bits,
            } => {
                format!(
                    "generic_ao{}_qo{}_pb{}",
                    acids.to_token_stream(),
                    q_scores.to_token_stream(),
                    position_bits.to_token_stream(),
                )
            }
            ModelItem::Light {
                acids,
                q_scores,
                position_bits,
                q_score_max,
            } => {
                format!(
                    "light_ao{}_qo{}_pb{}_qm{}",
                    acids.to_token_stream(),
                    q_scores.to_token_stream(),
                    position_bits.to_token_stream(),
                    q_score_max.to_token_stream(),
                )
            }
        }
    }
}

impl Parse for ModelItem {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        if ident == "dummy" {
            let _content;
            parenthesized!(_content in input);

            Ok(Self::Dummy)
        } else if ident == "generic" {
            let content;
            parenthesized!(content in input);
            let acids = content.parse::<Lit>()?;
            content.parse::<Token![,]>()?;
            let q_scores = content.parse::<Lit>()?;
            content.parse::<Token![,]>()?;
            let position_bits = content.parse::<Lit>()?;

            Ok(Self::Generic {
                acids,
                q_scores,
                position_bits,
            })
        } else if ident == "light" {
            let content;
            parenthesized!(content in input);
            let acids = content.parse::<Lit>()?;
            content.parse::<Token![,]>()?;
            let q_scores = content.parse::<Lit>()?;
            content.parse::<Token![,]>()?;
            let position_bits = content.parse::<Lit>()?;
            content.parse::<Token![,]>()?;
            let q_score_max = content.parse::<Lit>()?;

            Ok(Self::Light {
                acids,
                q_scores,
                position_bits,
                q_score_max,
            })
        } else {
            Err(syn::Error::new(
                ident.span(),
                "expected `dummy`, `generic`, or `light`",
            ))
        }
    }
}

struct Model {
    items: Punctuated<ModelItem, Token![,]>,
}

impl Parse for Model {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let items = input.parse_terminated(ModelItem::parse)?;

        Ok(Model { items })
    }
}

#[proc_macro]
pub fn model(input: TokenStream) -> TokenStream {
    let model = parse_macro_input!(input as Model);

    let enum_variants: Vec<proc_macro2::TokenStream> = model
        .items
        .iter()
        .map(|x| x.as_type_enum_variant())
        .collect();
    let name_variants: Vec<proc_macro2::TokenStream> =
        model.items.iter().map(|x| x.as_name_variant()).collect();
    let enum_values: Vec<proc_macro2::TokenStream> =
        model.items.iter().map(|x| x.as_enum_value()).collect();
    let variant_num = enum_values.len();
    let generator_variants: Vec<proc_macro2::TokenStream> = model
        .items
        .iter()
        .map(|x| x.as_generator_variant())
        .collect();
    let spec_num_variants: Vec<proc_macro2::TokenStream> = model
        .items
        .iter()
        .map(|x| x.as_spec_num_variant())
        .collect();

    let output = quote! {
        #[doc = "An exact type of a context specifier, which means how it is generated, using acids, quality scores, and position data."]
        #[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
        pub enum ContextSpecType {
            #(#enum_variants,)*
        }

        impl ContextSpecType {
            #[doc = "An array storing all possible enum variants."]
            pub const VALUES: [ContextSpecType; #variant_num] = [
                #(#enum_values,)*
            ];

            #[doc = "Returns the enum variant name for this context spec type."]
            #[must_use]
            pub fn name(&self) -> &'static str {
                match self {
                    #(#name_variants,)*
                }
            }

            #[doc = "Returns a context spec generator instance for this context spec type."]
            #[must_use]
            pub fn generator(&self, length: usize) -> Box<dyn ContextSpecGenerator> {
                match self {
                    #(#generator_variants)*
                }
            }

            #[doc = "Returns the maximum value of the context spec this type can produce."]
            #[must_use]
            pub fn spec_num(&self) -> u32 {
                match self {
                    #(#spec_num_variants)*
                }
            }
        }

        impl std::fmt::Display for ContextSpecType {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.name())
            }
        }
    };
    output.into()
}
