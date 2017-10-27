// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::cell::RefCell;
use std::collections::BTreeMap;

use ast;
use ast::{Ident, Name, TokenTree};
use codemap::Span;
use ext::base::{ExtCtxt, MacEager, MacResult};
use ext::build::AstBuilder;
use parse::token;
use ptr::P;
use util::small_vector::SmallVector;

// Maximum width of any line in an extended error description (inclusive).
const MAX_DESCRIPTION_WIDTH: usize = 80;

thread_local! {
    static REGISTERED_DIAGNOSTICS: RefCell<ErrorMap> = {
        RefCell::new(BTreeMap::new())
    }
}

/// Error information type.
pub struct ErrorInfo {
    pub description: Option<Name>,
    pub use_site: Option<Span>
}

/// Mapping from error codes to metadata.
pub type ErrorMap = BTreeMap<Name, ErrorInfo>;

fn with_registered_diagnostics<T, F>(f: F) -> T where
    F: FnOnce(&mut ErrorMap) -> T,
{
    REGISTERED_DIAGNOSTICS.with(move |slot| {
        f(&mut *slot.borrow_mut())
    })
}

pub fn expand_diagnostic_used<'cx>(ecx: &'cx mut ExtCtxt,
                                   span: Span,
                                   token_tree: &[TokenTree])
                                   -> Box<MacResult+'cx> {
    let code = match (token_tree.len(), token_tree.get(0)) {
        (1, Some(&ast::TtToken(_, token::Ident(code, _)))) => code,
        _ => unreachable!()
    };

    with_registered_diagnostics(|diagnostics| {
        match diagnostics.get_mut(&code.name) {
            // Previously used errors.
            Some(&mut ErrorInfo { description: _, use_site: Some(previous_span) }) => {
                ecx.span_warn(span, &format!(
                    "diagnostic code {} already used", &token::get_ident(code)
                ));
                ecx.span_note(previous_span, "previous invocation");
            }
            // Newly used errors.
            Some(ref mut info) => {
                info.use_site = Some(span);
            }
            // Unregistered errors.
            None => {
                ecx.span_err(span, &format!(
                    "used diagnostic code {} not registered", &token::get_ident(code)
                ));
            }
        }
    });
    MacEager::expr(ecx.expr_tuple(span, Vec::new()))
}

pub fn expand_register_diagnostic<'cx>(ecx: &'cx mut ExtCtxt,
                                       span: Span,
                                       token_tree: &[TokenTree])
                                       -> Box<MacResult+'cx> {
    let (code, description) = match (
        token_tree.len(),
        token_tree.get(0),
        token_tree.get(1),
        token_tree.get(2)
    ) {
        (1, Some(&ast::TtToken(_, token::Ident(ref code, _))), None, None) => {
            (code, None)
        },
        (3, Some(&ast::TtToken(_, token::Ident(ref code, _))),
            Some(&ast::TtToken(_, token::Comma)),
            Some(&ast::TtToken(_, token::Literal(token::StrRaw(description, _), None)))) => {
            (code, Some(description))
        }
        _ => unreachable!()
    };
    // Check that the description starts and ends with a newline and doesn't
    // overflow the maximum line width.
    description.map(|raw_msg| {
        let msg = raw_msg.as_str();
        if !msg.starts_with("\n") || !msg.ends_with("\n") {
            ecx.span_err(span, &format!(
                "description for error code {} doesn't start and end with a newline",
                token::get_ident(*code)
            ));
        }
        if msg.lines().any(|line| line.len() > MAX_DESCRIPTION_WIDTH) {
            ecx.span_err(span, &format!(
                "description for error code {} contains a line longer than {} characters",
                token::get_ident(*code), MAX_DESCRIPTION_WIDTH
            ));
        }
    });
    // Add the error to the map.
    with_registered_diagnostics(|diagnostics| {
        let info = ErrorInfo {
            description: description,
            use_site: None
        };
        if diagnostics.insert(code.name, info).is_some() {
            ecx.span_err(span, &format!(
                "diagnostic code {} already registered", &token::get_ident(*code)
            ));
        }
    });
    let sym = Ident::new(token::gensym(&(
        "__register_diagnostic_".to_string() + &token::get_ident(*code)
    )));
    MacEager::items(SmallVector::many(vec![
        ecx.item_mod(
            span,
            span,
            sym,
            Vec::new(),
            Vec::new()
        )
    ]))
}

pub fn expand_build_diagnostic_array<'cx>(ecx: &'cx mut ExtCtxt,
                                          span: Span,
                                          token_tree: &[TokenTree])
                                          -> Box<MacResult+'cx> {
    assert_eq!(token_tree.len(), 3);
    let (_crate_name, name) = match (&token_tree[0], &token_tree[2]) {
        (
            // Crate name.
            &ast::TtToken(_, token::Ident(ref crate_name, _)),
            // DIAGNOSTICS ident.
            &ast::TtToken(_, token::Ident(ref name, _))
        ) => (crate_name.as_str(), name),
        _ => unreachable!()
    };

    // FIXME (#25705): we used to ensure error code uniqueness and
    // output error description JSON metadata here, but the approach
    // employed was too brittle.

    // Construct the output expression.
    let (count, expr) =
        with_registered_diagnostics(|diagnostics| {
            let descriptions: Vec<P<ast::Expr>> =
                diagnostics.iter().filter_map(|(code, info)| {
                    info.description.map(|description| {
                        ecx.expr_tuple(span, vec![
                            ecx.expr_str(span, token::get_name(*code)),
                            ecx.expr_str(span, token::get_name(description))
                        ])
                    })
                }).collect();
            (descriptions.len(), ecx.expr_vec(span, descriptions))
        });

    let static_ = ecx.lifetime(span, ecx.name_of("'static"));
    let ty_str = ecx.ty_rptr(
        span,
        ecx.ty_ident(span, ecx.ident_of("str")),
        Some(static_),
        ast::MutImmutable,
    );

    let ty = ecx.ty(
        span,
        ast::TyFixedLengthVec(
            ecx.ty(
                span,
                ast::TyTup(vec![ty_str.clone(), ty_str])
            ),
            ecx.expr_usize(span, count),
        ),
    );

    MacEager::items(SmallVector::many(vec![
        P(ast::Item {
            ident: name.clone(),
            attrs: Vec::new(),
            id: ast::DUMMY_NODE_ID,
            node: ast::ItemStatic(
                ty,
                ast::MutImmutable,
                expr,
            ),
            vis: ast::Public,
            span: span,
        })
    ]))
}
