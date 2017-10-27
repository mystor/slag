// Copyright 2012-2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use ast::{MetaItem, Expr};
use codemap::Span;
use ext::base::{ExtCtxt, Annotatable};
use ext::build::AstBuilder;
use ext::deriving::generic::*;
use ext::deriving::generic::ty::*;
use parse::token::InternedString;
use ptr::P;

pub fn expand_deriving_default(cx: &mut ExtCtxt,
                               span: Span,
                               mitem: &MetaItem,
                               item: &Annotatable,
                               push: &mut FnMut(Annotatable))
{
    let inline = cx.meta_word(span, InternedString::new("inline"));
    let attrs = vec!(cx.attribute(span, inline));
    let trait_def = TraitDef {
        span: span,
        attributes: Vec::new(),
        path: path_std!(cx, core::default::Default),
        additional_bounds: Vec::new(),
        generics: LifetimeBounds::empty(),
        methods: vec!(
            MethodDef {
                name: "default",
                generics: LifetimeBounds::empty(),
                explicit_self: None,
                args: Vec::new(),
                ret_ty: Self_,
                attributes: attrs,
                is_unsafe: false,
                combine_substructure: combine_substructure(Box::new(|a, b, c| {
                    default_substructure(a, b, c)
                }))
            }
        ),
        associated_types: Vec::new(),
    };
    trait_def.expand(cx, mitem, item, push)
}

fn default_substructure(cx: &mut ExtCtxt, trait_span: Span, substr: &Substructure) -> P<Expr> {
    let default_ident = vec!(
        cx.ident_of_std("core"),
        cx.ident_of("default"),
        cx.ident_of("Default"),
        cx.ident_of("default")
    );
    let default_call = |span| cx.expr_call_global(span, default_ident.clone(), Vec::new());

    return match *substr.fields {
        StaticStruct(_, ref summary) => {
            match *summary {
                Unnamed(ref fields) => {
                    if fields.is_empty() {
                        cx.expr_ident(trait_span, substr.type_ident)
                    } else {
                        let exprs = fields.iter().map(|sp| default_call(*sp)).collect();
                        cx.expr_call_ident(trait_span, substr.type_ident, exprs)
                    }
                }
                Named(ref fields) => {
                    let default_fields = fields.iter().map(|&(ident, span)| {
                        cx.field_imm(span, ident, default_call(span))
                    }).collect();
                    cx.expr_struct_ident(trait_span, substr.type_ident, default_fields)
                }
            }
        }
        StaticEnum(..) => {
            cx.span_err(trait_span, "`Default` cannot be derived for enums, only structs");
            // let compilation continue
            cx.expr_usize(trait_span, 0)
        }
        _ => cx.span_bug(trait_span, "Non-static method in `derive(Default)`")
    };
}
