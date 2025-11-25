// src/tools/analyzer.rs
use std::path::PathBuf;
use anyhow::{Context, Result};
use tokio::fs;
use syn::{Item, Type, ReturnType, FnArg, Visibility};
use quote::ToTokens;
use rmcp::schemars::JsonSchema;
use serde::{Deserialize};
use rmcp::schemars;

#[derive(Deserialize, JsonSchema)]
pub struct AnalyzeRequest {
    #[schemars(description = "Absolute path to the Rust file")]
    pub path: String,
}

pub struct SymbolAnalyzer;

impl SymbolAnalyzer {
    pub fn new() -> Self {
        Self
    }

    pub async fn analyze(&self, path: PathBuf) -> Result<String> {
        if !path.exists() {
            anyhow::bail!("File '{}' does not exist", path.display());
        }

        let content = fs::read_to_string(&path)
            .await
            .context("Failed to read file")?;

        // Parse the file into an Abstract Syntax Tree (AST)
        let syntax = syn::parse_file(&content)
            .context("Failed to parse Rust code. Is the syntax valid?")?;

        let mut outline = String::new();
        outline.push_str(&format!("// OUTLINE: {}\n", path.display()));

        for item in syntax.items {
            match item {
                Item::Struct(s) => {
                    outline.push_str(&format!("\n{}struct {} {{\n", vis_to_string(&s.vis), s.ident));
                    for field in s.fields {
                        if let Some(ident) = field.ident {
                            let type_name = type_to_string(&field.ty);
                            outline.push_str(&format!("    {}: {},\n", ident, type_name));
                        }
                    }
                    outline.push_str("}\n");
                }
                Item::Enum(e) => {
                    outline.push_str(&format!("\n{}enum {} {{\n", vis_to_string(&e.vis), e.ident));
                    for variant in e.variants {
                        outline.push_str(&format!("    {},\n", variant.ident));
                    }
                    outline.push_str("}\n");
                }
                Item::Fn(f) => {
                    let sig = sig_to_string(&f.sig);
                    outline.push_str(&format!("\n{}{};\n", vis_to_string(&f.vis), sig));
                }
                Item::Impl(i) => {
                    let trait_part = if let Some((_, path, _)) = i.trait_ {
                        format!("{} for ", path.to_token_stream())
                    } else {
                        String::new()
                    };
                    let self_ty = type_to_string(&i.self_ty);

                    outline.push_str(&format!("\nimpl {}{} {{\n", trait_part, self_ty));

                    for item in i.items {
                        if let syn::ImplItem::Fn(method) = item {
                            let sig = sig_to_string(&method.sig);
                            outline.push_str(&format!("    {};\n", sig));
                        }
                    }
                    outline.push_str("}\n");
                }
                Item::Mod(m) => {
                    outline.push_str(&format!("\n{}mod {};\n", vis_to_string(&m.vis), m.ident));
                }
                Item::Use(u) => {
                    // Todo: -> Optional
                    // Optional: include imports? usually too noisy.
                    // outline.push_str(&format!("use ...;\n"));
                }
                _ => {} // Ignore macros, consts, externs for brevity
            }
        }

        Ok(outline)
    }
}

// --- Helper Functions ---

fn vis_to_string(vis: &Visibility) -> String {
    match vis {
        Visibility::Public(_) => "pub ".to_string(),
        Visibility::Restricted(_) => "pub(crate) ".to_string(), // Simplified
        Visibility::Inherited => "".to_string(),
    }
}

fn type_to_string(ty: &Type) -> String {
    // quote! turns the Type AST back into Rust code string
    let tokens = ty.to_token_stream();
    tokens.to_string().replace(" ", "") // Remove excess spaces
}

fn sig_to_string(sig: &syn::Signature) -> String {
    let name = &sig.ident;
    let async_prefix = if sig.asyncness.is_some() { "async " } else { "" };

    let inputs = sig.inputs.iter().map(|arg| {
        match arg {
            FnArg::Receiver(_) => "self".to_string(),
            FnArg::Typed(pat) => {
                let ty = type_to_string(&pat.ty);
                // We often don't need the variable name for high-level understanding,
                // but it helps. Let's keep it simple: just the type or name:type
                format!("{}", pat.to_token_stream())
            }
        }
    }).collect::<Vec<_>>().join(", ");

    let output = match &sig.output {
        ReturnType::Default => String::new(),
        ReturnType::Type(_, ty) => format!(" -> {}", type_to_string(ty)),
    };

    format!("{}fn {}({}){}", async_prefix, name, inputs, output)
}