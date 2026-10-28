use anyhow::{Context, Result};
use quote::ToTokens;
use std::fs;
use std::path::Path;
use syn::{ImplItem, Item, TraitItem};

#[derive(Debug, Default)]
pub struct FileSkeleton {
    pub items: Vec<String>,
}

pub fn extract_skeleton(path: &Path) -> Result<FileSkeleton> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read file {:?}", path))?;

    let syntax = syn::parse_file(&content)
        .with_context(|| format!("Failed to parse AST for {:?}", path))?;

    let mut items = Vec::new();

    for item in syntax.items {
        if let Some( formatted ) = format_item(&item) {
            items.push(formatted);
        }
    }

    Ok(FileSkeleton { items })
}

fn format_item(item: &Item) -> Option<String> {
    match item {
        Item::Use(u) => Some(u.to_token_stream().to_string()),
        Item::Struct(s) => Some(s.to_token_stream().to_string()),
        Item::Enum(e) => Some(e.to_token_stream().to_string()),
        Item::Type(t) => Some(t.to_token_stream().to_string()),
        Item::Const(c) => Some(c.to_token_stream().to_string()),
        Item::Mod(m) => {
            // Если модуль просто объявлен (например, `pub mod scanner;`), сохраняем как есть
            if m.content.is_none() {
                Some(m.to_token_stream().to_string())
            } else {
                Some(format!("{} mod {} {{ ... }}", m.vis.to_token_stream(), m.ident))
            }
        }
        Item::Fn(f) => {
            // Берем только видимость и сигнатуру функции, без тела (блока кода)
            let vis = f.vis.to_token_stream();
            let sig = f.sig.to_token_stream();
            Some(format!("{} {};", vis, sig).trim().to_string())
        }
        Item::Trait(t) => {
            let mut methods = Vec::new();
            for sub in &t.items {
                if let TraitItem::Fn(m) = sub {
                    methods.push(format!("    {};", m.sig.to_token_stream()));
                }
            }
            Some(format!(
                "{} trait {} {{\n{}\n}}",
                t.vis.to_token_stream(),
                t.ident,
                methods.join("\n")
            ).trim().to_string())
        }
                  Item::Impl(i) => {
            let self_ty = i.self_ty.to_token_stream();
            let trait_part = match &i.trait_ {
                Some(t) => format!("{} for ", t.0.to_token_stream()),
                None => String::new(),
            };

            let mut methods = Vec::new();
            for sub in &i.items {
                if let ImplItem::Fn(m) = sub {
                    let vis = m.vis.to_token_stream();
                    let sig = m.sig.to_token_stream();
                    methods.push(format!("    {} {};", vis, sig).trim_start().to_string());
                }
            }
            Some(format!(
                "impl {}{} {{\n    {}\n}}",
                trait_part,
                self_ty,
                methods.join("\n    ")
            ))
        }
        _ => None,
    }
}