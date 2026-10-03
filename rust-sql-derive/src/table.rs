use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, Fields, parse_macro_input};

pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;

    let fields = match input.data {
        syn::Data::Struct(data) => data.fields,

        _ => {
            return syn::Error::new_spanned(&name, "Table can only be derived for structs")
                .to_compile_error()
                .into();
        }
    };

    let (field_definitions, value_definitions, const_definitions, row_definitions) = match fields {
        Fields::Named(fields) => {
            let mut definitions = Vec::new();
            let mut values = Vec::new();
            let mut consts = Vec::new();
            let mut rows = Vec::new();

            let mut primary_key_count = 0;

            for (index, field) in fields.named.iter().enumerate() {
                let field_name = field.ident.as_ref().expect("named field");

                let column_name = field_name.to_string();

                let field_type = match RustFieldType::from_type(&field.ty) {
                    Ok(field_type) => field_type,

                    Err(error) => {
                        return syn::Error::new_spanned(&field.ty, error)
                            .to_compile_error()
                            .into();
                    }
                };

                let value_type = field_type.value_type();

                let column_type = field_type.column_type();

                let value = field_type.value(field_name);

                let row_value = field_type.parse_row_value(field_name, index);

                let primary_key = field
                    .attrs
                    .iter()
                    .any(|attr| attr.path().is_ident("primary_key"));

                let nullable = field_type.nullable();

                if primary_key && nullable {
                    return syn::Error::new_spanned(&field.ty, "Primary key cannot be nullable")
                        .to_compile_error()
                        .into();
                }

                if primary_key {
                    primary_key_count += 1;
                }

                let nullability_type = field_type.nullability_type();

                // Type-erased column metadata.
                definitions.push(quote! {
                    ::rust_sql::orm::ColumnRef::new(
                        #column_name,
                        #value_type,
                        #primary_key,
                        #nullable,
                    )
                });

                let column_ident = field_name.clone();

                // Type-safe column.
                consts.push(quote! {
                    #[allow(non_upper_case_globals)]
                    pub const #column_ident:
                        ::rust_sql::orm::Column<#column_type, #nullability_type> =
                        ::rust_sql::orm::Column::new(
                           #column_name,
                            #value_type,
                            #primary_key,
                            #nullable,
                        );
                });

                values.push(quote! {
                    (
                        #column_name,
                        #value,
                    )
                });

                rows.push(quote! {
                    #field_name: #row_value
                });
            }

            if primary_key_count == 0 {
                return syn::Error::new_spanned(&name, "Primary key not found")
                    .to_compile_error()
                    .into();
            }

            if primary_key_count > 1 {
                return syn::Error::new_spanned(&name, "Multiple primary keys are not supported")
                    .to_compile_error()
                    .into();
            }

            (definitions, values, consts, rows)
        }

        Fields::Unnamed(_) => {
            return syn::Error::new_spanned(&name, "Table does not support tuple structs")
                .to_compile_error()
                .into();
        }

        Fields::Unit => {
            return syn::Error::new_spanned(&name, "Table does not support unit structs")
                .to_compile_error()
                .into();
        }
    };

    let table_name = name.to_string().to_lowercase();

    let expanded = quote! {
        impl #name {
            pub const TABLE_NAME: &'static str =
                #table_name;

            #(#const_definitions)*

            pub fn fields()
                -> Vec<::rust_sql::orm::ColumnRef>
            {
                vec![
                    #(#field_definitions),*
                ]
            }

            pub fn values(
                &self,
            ) -> Vec<(
                &'static str,
                ::rust_sql::Value,
            )> {
                vec![
                    #(#value_definitions),*
                ]
            }
        }

        impl ::rust_sql::orm::Table for #name {
            fn table_name() -> &'static str {
                Self::TABLE_NAME
            }

            fn fields()
                -> Vec<::rust_sql::orm::ColumnRef>
            {
                Self::fields()
            }

            fn values(
                &self,
            ) -> Vec<(
                &'static str,
                ::rust_sql::Value,
            )> {
                Self::values(self)
            }

            fn from_row(
                row: &::rust_sql::ResultRow,
            ) -> Result<Self, ::rust_sql::DbError> {
                Ok(Self {
                    #(#row_definitions),*
                })
            }
        }
    };

    TokenStream::from(expanded)
}

enum RustFieldType {
    Int {
        nullable: bool,
        ty: proc_macro2::TokenStream,
    },

    UInt {
        nullable: bool,
        ty: proc_macro2::TokenStream,
    },

    Float {
        nullable: bool,
        ty: proc_macro2::TokenStream,
    },

    Bool {
        nullable: bool,
        ty: proc_macro2::TokenStream,
    },

    String {
        nullable: bool,
        ty: proc_macro2::TokenStream,
    },

    Bytes {
        nullable: bool,
        ty: proc_macro2::TokenStream,
    },
}

impl RustFieldType {
    fn from_type(ty: &syn::Type) -> Result<Self, &'static str> {
        let syn::Type::Path(type_path) = ty else {
            return Err("Unsupported field type");
        };

        let Some(segment) = type_path.path.segments.last() else {
            return Err("Unsupported field type");
        };

        if segment.ident == "Option" {
            let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
                return Err("Invalid Option type");
            };

            let Some(syn::GenericArgument::Type(inner_type)) = arguments.args.first() else {
                return Err("Invalid Option type");
            };

            return Self::from_inner_type(inner_type, true);
        }

        Self::from_inner_type(ty, false)
    }

    fn from_inner_type(ty: &syn::Type, nullable: bool) -> Result<Self, &'static str> {
        let syn::Type::Path(type_path) = ty else {
            return Err("Unsupported field type");
        };

        let Some(segment) = type_path.path.segments.last() else {
            return Err("Unsupported field type");
        };

        let ty_tokens = quote! { #ty };

        match segment.ident.to_string().as_str() {
            "i8" | "i16" | "i32" | "i64" | "isize" => Ok(Self::Int {
                nullable,
                ty: ty_tokens,
            }),

            "u8" | "u16" | "u32" | "u64" | "usize" => Ok(Self::UInt {
                nullable,
                ty: ty_tokens,
            }),

            "f32" | "f64" => Ok(Self::Float {
                nullable,
                ty: ty_tokens,
            }),

            "bool" => Ok(Self::Bool {
                nullable,
                ty: ty_tokens,
            }),

            "String" => Ok(Self::String {
                nullable,
                ty: ty_tokens,
            }),

            "Vec" => Ok(Self::Bytes {
                nullable,
                ty: ty_tokens,
            }),

            _ => Err("Unsupported field type"),
        }
    }

    fn parse_row_value(&self, field_name: &syn::Ident, index: usize) -> proc_macro2::TokenStream {
        let field = field_name.to_string();

        match self {
            Self::Int {
                nullable: false,
                ty,
            } => {
                quote! {
                    match row.values().get(#index) {
                        Some(::rust_sql::Value::Int(value)) => {
                            *value as #ty
                        }

                        Some(value) => {
                            return Err(::rust_sql::DbError::new(
                                format!(
                                    "Expected Int for field {}, got {:?}",
                                    #field,
                                    value,
                                )
                            ));
                        }

                        None => {
                            return Err(::rust_sql::DbError::new(
                                format!("Missing value for field {}", #field)
                            ));
                        }
                    }
                }
            }

            Self::Int { nullable: true, ty } => {
                quote! {
                    match row.values().get(#index) {
                        Some(::rust_sql::Value::Null) => None,

                        Some(::rust_sql::Value::Int(value)) => {
                            Some(*value as #ty)
                        }

                        Some(value) => {
                            return Err(::rust_sql::DbError::new(
                                format!(
                                    "Expected Int or Null for field {}, got {:?}",
                                    #field,
                                    value,
                                )
                            ));
                        }

                        None => {
                            return Err(::rust_sql::DbError::new(
                                format!("Missing value for field {}", #field)
                            ));
                        }
                    }
                }
            }

            Self::UInt {
                nullable: false,
                ty,
            } => {
                quote! {
                    match row.values().get(#index) {
                        Some(::rust_sql::Value::UInt(value)) => {
                            *value as #ty
                        }

                        Some(::rust_sql::Value::Int(value)) if *value >= 0 => {
                            *value as #ty
                        }

                        Some(value) => {
                            return Err(::rust_sql::DbError::new(
                                format!(
                                    "Expected UInt for field {}, got {:?}",
                                    #field,
                                    value,
                                )
                            ));
                        }

                        None => {
                            return Err(::rust_sql::DbError::new(
                                format!("Missing value for field {}", #field)
                            ));
                        }
                    }
                }
            }

            Self::UInt { nullable: true, ty } => {
                quote! {
                    match row.values().get(#index) {
                        Some(::rust_sql::Value::Null) => None,

                        Some(::rust_sql::Value::UInt(value)) => {
                            Some(*value as #ty)
                        }

                        Some(::rust_sql::Value::Int(value)) if *value >= 0 => {
                            Some(*value as #ty)
                        }

                        Some(value) => {
                            return Err(::rust_sql::DbError::new(
                                format!(
                                    "Expected UInt or Null for field {}, got {:?}",
                                    #field,
                                    value,
                                )
                            ));
                        }

                        None => {
                            return Err(::rust_sql::DbError::new(
                                format!("Missing value for field {}", #field)
                            ));
                        }
                    }
                }
            }

            Self::Float {
                nullable: false,
                ty,
            } => {
                quote! {
                    match row.values().get(#index) {
                        Some(::rust_sql::Value::Float(value)) => {
                            *value as #ty
                        }

                        Some(value) => {
                            return Err(::rust_sql::DbError::new(
                                format!(
                                    "Expected Float for field {}, got {:?}",
                                    #field,
                                    value,
                                )
                            ));
                        }

                        None => {
                            return Err(::rust_sql::DbError::new(
                                format!("Missing value for field {}", #field)
                            ));
                        }
                    }
                }
            }

            Self::Float { nullable: true, ty } => {
                quote! {
                    match row.values().get(#index) {
                        Some(::rust_sql::Value::Null) => None,

                        Some(::rust_sql::Value::Float(value)) => {
                            Some(*value as #ty)
                        }

                        Some(value) => {
                            return Err(::rust_sql::DbError::new(
                                format!(
                                    "Expected Float or Null for field {}, got {:?}",
                                    #field,
                                    value,
                                )
                            ));
                        }

                        None => {
                            return Err(::rust_sql::DbError::new(
                                format!("Missing value for field {}", #field)
                            ));
                        }
                    }
                }
            }

            Self::Bool {
                nullable: false, ..
            } => {
                quote! {
                    match row.values().get(#index) {
                        Some(::rust_sql::Value::Bool(value)) => *value,

                        Some(::rust_sql::Value::Int(value)) => {
                            *value != 0
                        }

                        Some(value) => {
                            return Err(::rust_sql::DbError::new(
                                format!(
                                    "Expected Bool for field {}, got {:?}",
                                    #field,
                                    value,
                                )
                            ));
                        }

                        None => {
                            return Err(::rust_sql::DbError::new(
                                format!("Missing value for field {}", #field)
                            ));
                        }
                    }
                }
            }

            Self::Bool { nullable: true, .. } => {
                quote! {
                    match row.values().get(#index) {
                        Some(::rust_sql::Value::Null) => None,

                        Some(::rust_sql::Value::Bool(value)) => {
                            Some(*value)
                        }

                        Some(::rust_sql::Value::Int(value)) => {
                            Some(*value != 0)
                        }

                        Some(value) => {
                            return Err(::rust_sql::DbError::new(
                                format!(
                                    "Expected Bool or Null for field {}, got {:?}",
                                    #field,
                                    value,
                                )
                            ));
                        }

                        None => {
                            return Err(::rust_sql::DbError::new(
                                format!("Missing value for field {}", #field)
                            ));
                        }
                    }
                }
            }

            Self::String {
                nullable: false, ..
            } => {
                quote! {
                    match row.values().get(#index) {
                        Some(::rust_sql::Value::String(value)) => {
                            value.clone()
                        }

                        Some(value) => {
                            return Err(::rust_sql::DbError::new(
                                format!(
                                    "Expected String for field {}, got {:?}",
                                    #field,
                                    value,
                                )
                            ));
                        }

                        None => {
                            return Err(::rust_sql::DbError::new(
                                format!("Missing value for field {}", #field)
                            ));
                        }
                    }
                }
            }

            Self::String { nullable: true, .. } => {
                quote! {
                    match row.values().get(#index) {
                        Some(::rust_sql::Value::Null) => None,

                        Some(::rust_sql::Value::String(value)) => {
                            Some(value.clone())
                        }

                        Some(value) => {
                            return Err(::rust_sql::DbError::new(
                                format!(
                                    "Expected String or Null for field {}, got {:?}",
                                    #field,
                                    value,
                                )
                            ));
                        }

                        None => {
                            return Err(::rust_sql::DbError::new(
                                format!("Missing value for field {}", #field)
                            ));
                        }
                    }
                }
            }

            Self::Bytes {
                nullable: false, ..
            } => {
                quote! {
                    match row.values().get(#index) {
                        Some(::rust_sql::Value::Bytes(value)) => {
                            value.clone()
                        }

                        Some(value) => {
                            return Err(::rust_sql::DbError::new(
                                format!(
                                    "Expected Bytes for field {}, got {:?}",
                                    #field,
                                    value,
                                )
                            ));
                        }

                        None => {
                            return Err(::rust_sql::DbError::new(
                                format!("Missing value for field {}", #field)
                            ));
                        }
                    }
                }
            }

            Self::Bytes { nullable: true, .. } => {
                quote! {
                    match row.values().get(#index) {
                        Some(::rust_sql::Value::Null) => None,

                        Some(::rust_sql::Value::Bytes(value)) => {
                            Some(value.clone())
                        }

                        Some(value) => {
                            return Err(::rust_sql::DbError::new(
                                format!(
                                    "Expected Bytes or Null for field {}, got {:?}",
                                    #field,
                                    value,
                                )
                            ));
                        }

                        None => {
                            return Err(::rust_sql::DbError::new(
                                format!("Missing value for field {}", #field)
                            ));
                        }
                    }
                }
            }
        }
    }

    fn column_type(&self) -> proc_macro2::TokenStream {
        match self {
            Self::Int { ty, .. }
            | Self::UInt { ty, .. }
            | Self::Float { ty, .. }
            | Self::Bool { ty, .. }
            | Self::String { ty, .. }
            | Self::Bytes { ty, .. } => ty.clone(),
        }
    }

    fn value_type(&self) -> proc_macro2::TokenStream {
        match self {
            Self::Int { .. } => {
                quote! {
                    ::rust_sql::ValueType::Int
                }
            }

            Self::UInt { .. } => {
                quote! {
                    ::rust_sql::ValueType::UInt
                }
            }

            Self::Float { .. } => {
                quote! {
                    ::rust_sql::ValueType::Float
                }
            }

            Self::Bool { .. } => {
                quote! {
                    ::rust_sql::ValueType::Bool
                }
            }

            Self::String { .. } => {
                quote! {
                    ::rust_sql::ValueType::String
                }
            }

            Self::Bytes { .. } => {
                quote! {
                    ::rust_sql::ValueType::Bytes
                }
            }
        }
    }

    fn value(&self, field_name: &syn::Ident) -> proc_macro2::TokenStream {
        match self {
            Self::Int {
                nullable: false, ..
            } => {
                quote! {
                    ::rust_sql::Value::Int(
                        self.#field_name as i64
                    )
                }
            }

            Self::Int { nullable: true, .. } => {
                quote! {
                    match &self.#field_name {
                        Some(value) => {
                            ::rust_sql::Value::Int(
                                *value as i64
                            )
                        }

                        None => {
                            ::rust_sql::Value::Null
                        }
                    }
                }
            }

            Self::UInt {
                nullable: false, ..
            } => {
                quote! {
                    ::rust_sql::Value::UInt(
                        self.#field_name as u64
                    )
                }
            }

            Self::UInt { nullable: true, .. } => {
                quote! {
                    match &self.#field_name {
                        Some(value) => {
                            ::rust_sql::Value::UInt(
                                *value as u64
                            )
                        }

                        None => {
                            ::rust_sql::Value::Null
                        }
                    }
                }
            }

            Self::Float {
                nullable: false, ..
            } => {
                quote! {
                    ::rust_sql::Value::Float(
                        self.#field_name as f64
                    )
                }
            }

            Self::Float { nullable: true, .. } => {
                quote! {
                    match &self.#field_name {
                        Some(value) => {
                            ::rust_sql::Value::Float(
                                *value as f64
                            )
                        }

                        None => {
                            ::rust_sql::Value::Null
                        }
                    }
                }
            }

            Self::Bool {
                nullable: false, ..
            } => {
                quote! {
                    ::rust_sql::Value::Bool(
                        self.#field_name
                    )
                }
            }

            Self::Bool { nullable: true, .. } => {
                quote! {
                    match &self.#field_name {
                        Some(value) => {
                            ::rust_sql::Value::Bool(
                                *value
                            )
                        }

                        None => {
                            ::rust_sql::Value::Null
                        }
                    }
                }
            }

            Self::String {
                nullable: false, ..
            } => {
                quote! {
                    ::rust_sql::Value::String(
                        self.#field_name.clone()
                    )
                }
            }

            Self::String { nullable: true, .. } => {
                quote! {
                    match &self.#field_name {
                        Some(value) => {
                            ::rust_sql::Value::String(
                                value.clone()
                            )
                        }

                        None => {
                            ::rust_sql::Value::Null
                        }
                    }
                }
            }

            Self::Bytes {
                nullable: false, ..
            } => {
                quote! {
                    ::rust_sql::Value::Bytes(
                        self.#field_name.clone()
                    )
                }
            }

            Self::Bytes { nullable: true, .. } => {
                quote! {
                    match &self.#field_name {
                        Some(value) => {
                            ::rust_sql::Value::Bytes(
                                value.clone()
                            )
                        }

                        None => {
                            ::rust_sql::Value::Null
                        }
                    }
                }
            }
        }
    }

    fn nullable(&self) -> bool {
        match self {
            Self::Int { nullable, .. }
            | Self::UInt { nullable, .. }
            | Self::Float { nullable, .. }
            | Self::Bool { nullable, .. }
            | Self::String { nullable, .. }
            | Self::Bytes { nullable, .. } => *nullable,
        }
    }

    fn nullability_type(&self) -> proc_macro2::TokenStream {
        match self.nullable() {
            true => quote! {
                ::rust_sql::orm::Nullable
            },

            false => quote! {
                ::rust_sql::orm::NotNullable
            },
        }
    }
}
