// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use ast::*;
use ast;
use ast_util;
use codemap;
use codemap::Span;
use owned_slice::OwnedSlice;
use parse::token;
use print::pprust;
use ptr::P;
use visit::Visitor;
use visit;

use std::cmp;
use std::u32;

pub fn path_name_i(idents: &[Ident]) -> String {
    // FIXME: Bad copies (#2543 -- same for everything else that says "bad")
    idents.iter().map(|i| {
        token::get_ident(*i).to_string()
    }).collect::<Vec<String>>().connect("::")
}

pub fn local_def(id: NodeId) -> DefId {
    ast::DefId { krate: LOCAL_CRATE, node: id }
}

pub fn is_local(did: ast::DefId) -> bool { did.krate == LOCAL_CRATE }

pub fn stmt_id(s: &Stmt) -> NodeId {
    match s.node {
      StmtDecl(_, id) => id,
      StmtExpr(_, id) => id,
      StmtSemi(_, id) => id,
      StmtMac(..) => panic!("attempted to analyze unexpanded stmt")
    }
}

pub fn binop_to_string(op: BinOp_) -> &'static str {
    match op {
        BiAdd => "+",
        BiSub => "-",
        BiMul => "*",
        BiDiv => "/",
        BiRem => "%",
        BiAnd => "&&",
        BiOr => "||",
        BiBitXor => "^",
        BiBitAnd => "&",
        BiBitOr => "|",
        BiShl => "<<",
        BiShr => ">>",
        BiEq => "==",
        BiLt => "<",
        BiLe => "<=",
        BiNe => "!=",
        BiGe => ">=",
        BiGt => ">"
    }
}

pub fn lazy_binop(b: BinOp_) -> bool {
    match b {
      BiAnd => true,
      BiOr => true,
      _ => false
    }
}

pub fn is_shift_binop(b: BinOp_) -> bool {
    match b {
      BiShl => true,
      BiShr => true,
      _ => false
    }
}

pub fn is_comparison_binop(b: BinOp_) -> bool {
    match b {
        BiEq | BiLt | BiLe | BiNe | BiGt | BiGe =>
            true,
        BiAnd | BiOr | BiAdd | BiSub | BiMul | BiDiv | BiRem |
        BiBitXor | BiBitAnd | BiBitOr | BiShl | BiShr =>
            false,
    }
}

/// Returns `true` if the binary operator takes its arguments by value
pub fn is_by_value_binop(b: BinOp_) -> bool {
    !is_comparison_binop(b)
}

/// Returns `true` if the unary operator takes its argument by value
pub fn is_by_value_unop(u: UnOp) -> bool {
    match u {
        UnNeg | UnNot => true,
        _ => false,
    }
}

pub fn unop_to_string(op: UnOp) -> &'static str {
    match op {
      UnUniq => "box() ",
      UnDeref => "*",
      UnNot => "!",
      UnNeg => "-",
    }
}

pub fn is_path(e: P<Expr>) -> bool {
    match e.node { ExprPath(..) => true, _ => false }
}

/// Get a string representation of a signed int type, with its value.
/// We want to avoid "45int" and "-3int" in favor of "45" and "-3"
pub fn int_ty_to_string(t: IntTy, val: Option<i64>) -> String {
    let s = match t {
        TyIs => "isize",
        TyI8 => "i8",
        TyI16 => "i16",
        TyI32 => "i32",
        TyI64 => "i64"
    };

    match val {
        // cast to a u64 so we can correctly print INT64_MIN. All integral types
        // are parsed as u64, so we wouldn't want to print an extra negative
        // sign.
        Some(n) => format!("{}{}", n as u64, s),
        None => s.to_string()
    }
}

pub fn int_ty_max(t: IntTy) -> u64 {
    match t {
        TyI8 => 0x80,
        TyI16 => 0x8000,
        TyIs | TyI32 => 0x80000000, // actually ni about TyIs
        TyI64 => 0x8000000000000000
    }
}

/// Get a string representation of an unsigned int type, with its value.
/// We want to avoid "42u" in favor of "42us". "42uint" is right out.
pub fn uint_ty_to_string(t: UintTy, val: Option<u64>) -> String {
    let s = match t {
        TyUs => "usize",
        TyU8 => "u8",
        TyU16 => "u16",
        TyU32 => "u32",
        TyU64 => "u64"
    };

    match val {
        Some(n) => format!("{}{}", n, s),
        None => s.to_string()
    }
}

pub fn uint_ty_max(t: UintTy) -> u64 {
    match t {
        TyU8 => 0xff,
        TyU16 => 0xffff,
        TyUs | TyU32 => 0xffffffff, // actually ni about TyUs
        TyU64 => 0xffffffffffffffff
    }
}

pub fn float_ty_to_string(t: FloatTy) -> String {
    match t {
        TyF32 => "f32".to_string(),
        TyF64 => "f64".to_string(),
    }
}

// convert a span and an identifier to the corresponding
// 1-segment path
pub fn ident_to_path(s: Span, identifier: Ident) -> Path {
    ast::Path {
        span: s,
        global: false,
        segments: vec!(
            ast::PathSegment {
                identifier: identifier,
                parameters: ast::AngleBracketedParameters(ast::AngleBracketedParameterData {
                    lifetimes: Vec::new(),
                    types: OwnedSlice::empty(),
                    bindings: OwnedSlice::empty(),
                })
            }
        ),
    }
}

// If path is a single segment ident path, return that ident. Otherwise, return
// None.
pub fn path_to_ident(path: &Path) -> Option<Ident> {
    if path.segments.len() != 1 {
        return None;
    }

    let segment = &path.segments[0];
    if !segment.parameters.is_empty() {
        return None;
    }

    Some(segment.identifier)
}

pub fn ident_to_pat(id: NodeId, s: Span, i: Ident) -> P<Pat> {
    P(Pat {
        id: id,
        node: PatIdent(BindByValue(MutImmutable), codemap::Spanned{span:s, node:i}, None),
        span: s
    })
}

pub fn name_to_dummy_lifetime(name: Name) -> Lifetime {
    Lifetime { id: DUMMY_NODE_ID,
               span: codemap::DUMMY_SP,
               name: name }
}

/// Generate a "pretty" name for an `impl` from its type and trait.
/// This is designed so that symbols of `impl`'d methods give some
/// hint of where they came from, (previously they would all just be
/// listed as `__extensions__::method_name::hash`, with no indication
/// of the type).
pub fn impl_pretty_name(trait_ref: &Option<TraitRef>, ty: Option<&Ty>) -> Ident {
    let mut pretty = match ty {
        Some(t) => pprust::ty_to_string(t),
        None => String::from("..")
    };

    match *trait_ref {
        Some(ref trait_ref) => {
            pretty.push('.');
            pretty.push_str(&pprust::path_to_string(&trait_ref.path));
        }
        None => {}
    }
    token::gensym_ident(&pretty[..])
}

pub fn struct_field_visibility(field: ast::StructField) -> Visibility {
    match field.node.kind {
        ast::NamedField(_, v) | ast::UnnamedField(v) => v
    }
}

/// Maps a binary operator to its precedence
pub fn operator_prec(op: ast::BinOp_) -> usize {
  match op {
      // 'as' sits here with 12
      BiMul | BiDiv | BiRem     => 11,
      BiAdd | BiSub             => 10,
      BiShl | BiShr             =>  9,
      BiBitAnd                  =>  8,
      BiBitXor                  =>  7,
      BiBitOr                   =>  6,
      BiLt | BiLe | BiGe | BiGt | BiEq | BiNe => 3,
      BiAnd                     =>  2,
      BiOr                      =>  1
  }
}

/// Precedence of the `as` operator, which is a binary operator
/// not appearing in the prior table.
pub const AS_PREC: usize = 12;

pub fn empty_generics() -> Generics {
    Generics {
        lifetimes: Vec::new(),
        ty_params: OwnedSlice::empty(),
        where_clause: WhereClause {
            id: DUMMY_NODE_ID,
            predicates: Vec::new(),
        }
    }
}

// ______________________________________________________________________
// Enumerating the IDs which appear in an AST

#[derive(Copy, Clone, RustcEncodable, RustcDecodable, Debug)]
pub struct IdRange {
    pub min: NodeId,
    pub max: NodeId,
}

impl IdRange {
    pub fn max() -> IdRange {
        IdRange {
            min: u32::MAX,
            max: u32::MIN,
        }
    }

    pub fn empty(&self) -> bool {
        self.min >= self.max
    }

    pub fn add(&mut self, id: NodeId) {
        self.min = cmp::min(self.min, id);
        self.max = cmp::max(self.max, id + 1);
    }
}

pub trait IdVisitingOperation {
    fn visit_id(&mut self, node_id: NodeId);
}

/// A visitor that applies its operation to all of the node IDs
/// in a visitable thing.

pub struct IdVisitor<'a, O:'a> {
    pub operation: &'a mut O,
    pub pass_through_items: bool,
    pub visited_outermost: bool,
}

impl<'a, O: IdVisitingOperation> IdVisitor<'a, O> {
    fn visit_generics_helper(&mut self, generics: &Generics) {
        for type_parameter in &*generics.ty_params {
            self.operation.visit_id(type_parameter.id)
        }
        for lifetime in &generics.lifetimes {
            self.operation.visit_id(lifetime.lifetime.id)
        }
    }
}

impl<'a, 'v, O: IdVisitingOperation> Visitor<'v> for IdVisitor<'a, O> {
    fn visit_mod(&mut self,
                 module: &Mod,
                 _: Span,
                 node_id: NodeId) {
        self.operation.visit_id(node_id);
        visit::walk_mod(self, module)
    }

    fn visit_foreign_item(&mut self, foreign_item: &ForeignItem) {
        self.operation.visit_id(foreign_item.id);
        visit::walk_foreign_item(self, foreign_item)
    }

    fn visit_item(&mut self, item: &Item) {
        if !self.pass_through_items {
            if self.visited_outermost {
                return
            } else {
                self.visited_outermost = true
            }
        }

        self.operation.visit_id(item.id);
        match item.node {
            ItemUse(ref view_path) => {
                match view_path.node {
                    ViewPathSimple(_, _) |
                    ViewPathGlob(_) => {}
                    ViewPathList(_, ref paths) => {
                        for path in paths {
                            self.operation.visit_id(path.node.id())
                        }
                    }
                }
            }
            ItemEnum(ref enum_definition, _) => {
                for variant in &enum_definition.variants {
                    self.operation.visit_id(variant.node.id)
                }
            }
            _ => {}
        }

        visit::walk_item(self, item);

        self.visited_outermost = false
    }

    fn visit_local(&mut self, local: &Local) {
        self.operation.visit_id(local.id);
        visit::walk_local(self, local)
    }

    fn visit_block(&mut self, block: &Block) {
        self.operation.visit_id(block.id);
        visit::walk_block(self, block)
    }

    fn visit_stmt(&mut self, statement: &Stmt) {
        self.operation.visit_id(ast_util::stmt_id(statement));
        visit::walk_stmt(self, statement)
    }

    fn visit_pat(&mut self, pattern: &Pat) {
        self.operation.visit_id(pattern.id);
        visit::walk_pat(self, pattern)
    }

    fn visit_expr(&mut self, expression: &Expr) {
        self.operation.visit_id(expression.id);
        visit::walk_expr(self, expression)
    }

    fn visit_ty(&mut self, typ: &Ty) {
        self.operation.visit_id(typ.id);
        visit::walk_ty(self, typ)
    }

    fn visit_generics(&mut self, generics: &Generics) {
        self.visit_generics_helper(generics);
        visit::walk_generics(self, generics)
    }

    fn visit_fn(&mut self,
                function_kind: visit::FnKind<'v>,
                function_declaration: &'v FnDecl,
                block: &'v Block,
                span: Span,
                node_id: NodeId) {
        if !self.pass_through_items {
            match function_kind {
                visit::FkMethod(..) if self.visited_outermost => return,
                visit::FkMethod(..) => self.visited_outermost = true,
                _ => {}
            }
        }

        self.operation.visit_id(node_id);

        match function_kind {
            visit::FkItemFn(_, generics, _, _, _, _) => {
                self.visit_generics_helper(generics)
            }
            visit::FkMethod(_, sig, _) => {
                self.visit_generics_helper(&sig.generics)
            }
            visit::FkFnBlock => {}
        }

        for argument in &function_declaration.inputs {
            self.operation.visit_id(argument.id)
        }

        visit::walk_fn(self,
                       function_kind,
                       function_declaration,
                       block,
                       span);

        if !self.pass_through_items {
            if let visit::FkMethod(..) = function_kind {
                self.visited_outermost = false;
            }
        }
    }

    fn visit_struct_field(&mut self, struct_field: &StructField) {
        self.operation.visit_id(struct_field.node.id);
        visit::walk_struct_field(self, struct_field)
    }

    fn visit_struct_def(&mut self,
                        struct_def: &StructDef,
                        _: ast::Ident,
                        _: &ast::Generics,
                        id: NodeId) {
        self.operation.visit_id(id);
        struct_def.ctor_id.map(|ctor_id| self.operation.visit_id(ctor_id));
        visit::walk_struct_def(self, struct_def);
    }

    fn visit_trait_item(&mut self, ti: &ast::TraitItem) {
        self.operation.visit_id(ti.id);
        visit::walk_trait_item(self, ti);
    }

    fn visit_impl_item(&mut self, ii: &ast::ImplItem) {
        self.operation.visit_id(ii.id);
        visit::walk_impl_item(self, ii);
    }

    fn visit_lifetime_ref(&mut self, lifetime: &Lifetime) {
        self.operation.visit_id(lifetime.id);
    }

    fn visit_lifetime_def(&mut self, def: &LifetimeDef) {
        self.visit_lifetime_ref(&def.lifetime);
    }

    fn visit_trait_ref(&mut self, trait_ref: &TraitRef) {
        self.operation.visit_id(trait_ref.ref_id);
        visit::walk_trait_ref(self, trait_ref);
    }
}

pub fn visit_ids_for_inlined_item<O: IdVisitingOperation>(item: &InlinedItem,
                                                          operation: &mut O) {
    let mut id_visitor = IdVisitor {
        operation: operation,
        pass_through_items: true,
        visited_outermost: false,
    };

    visit::walk_inlined_item(&mut id_visitor, item);
}

struct IdRangeComputingVisitor {
    result: IdRange,
}

impl IdVisitingOperation for IdRangeComputingVisitor {
    fn visit_id(&mut self, id: NodeId) {
        self.result.add(id);
    }
}

pub fn compute_id_range_for_inlined_item(item: &InlinedItem) -> IdRange {
    let mut visitor = IdRangeComputingVisitor {
        result: IdRange::max()
    };
    visit_ids_for_inlined_item(item, &mut visitor);
    visitor.result
}

/// Computes the id range for a single fn body, ignoring nested items.
pub fn compute_id_range_for_fn_body(fk: visit::FnKind,
                                    decl: &FnDecl,
                                    body: &Block,
                                    sp: Span,
                                    id: NodeId)
                                    -> IdRange
{
    let mut visitor = IdRangeComputingVisitor {
        result: IdRange::max()
    };
    let mut id_visitor = IdVisitor {
        operation: &mut visitor,
        pass_through_items: false,
        visited_outermost: false,
    };
    id_visitor.visit_fn(fk, decl, body, sp, id);
    id_visitor.operation.result
}

pub fn walk_pat<F>(pat: &Pat, mut it: F) -> bool where F: FnMut(&Pat) -> bool {
    // FIXME(#19596) this is a workaround, but there should be a better way
    fn walk_pat_<G>(pat: &Pat, it: &mut G) -> bool where G: FnMut(&Pat) -> bool {
        if !(*it)(pat) {
            return false;
        }

        match pat.node {
            PatIdent(_, _, Some(ref p)) => walk_pat_(&**p, it),
            PatStruct(_, ref fields, _) => {
                fields.iter().all(|field| walk_pat_(&*field.node.pat, it))
            }
            PatEnum(_, Some(ref s)) | PatTup(ref s) => {
                s.iter().all(|p| walk_pat_(&**p, it))
            }
            PatBox(ref s) | PatRegion(ref s, _) => {
                walk_pat_(&**s, it)
            }
            PatVec(ref before, ref slice, ref after) => {
                before.iter().all(|p| walk_pat_(&**p, it)) &&
                slice.iter().all(|p| walk_pat_(&**p, it)) &&
                after.iter().all(|p| walk_pat_(&**p, it))
            }
            PatMac(_) => panic!("attempted to analyze unexpanded pattern"),
            PatWild(_) | PatLit(_) | PatRange(_, _) | PatIdent(_, _, _) |
            PatEnum(_, _) | PatQPath(_, _) => {
                true
            }
        }
    }

    walk_pat_(pat, &mut it)
}

/// Returns true if the given struct def is tuple-like; i.e. that its fields
/// are unnamed.
pub fn struct_def_is_tuple_like(struct_def: &ast::StructDef) -> bool {
    struct_def.ctor_id.is_some()
}

/// Returns true if the given pattern consists solely of an identifier
/// and false otherwise.
pub fn pat_is_ident(pat: P<ast::Pat>) -> bool {
    match pat.node {
        ast::PatIdent(..) => true,
        _ => false,
    }
}

// are two paths equal when compared unhygienically?
// since I'm using this to replace ==, it seems appropriate
// to compare the span, global, etc. fields as well.
pub fn path_name_eq(a : &ast::Path, b : &ast::Path) -> bool {
    (a.span == b.span)
    && (a.global == b.global)
    && (segments_name_eq(&a.segments[..], &b.segments[..]))
}

// are two arrays of segments equal when compared unhygienically?
pub fn segments_name_eq(a : &[ast::PathSegment], b : &[ast::PathSegment]) -> bool {
    a.len() == b.len() &&
    a.iter().zip(b.iter()).all(|(s, t)| {
        s.identifier.name == t.identifier.name &&
        // FIXME #7743: ident -> name problems in lifetime comparison?
        // can types contain idents?
        s.parameters == t.parameters
    })
}

/// Returns true if this literal is a string and false otherwise.
pub fn lit_is_str(lit: &Lit) -> bool {
    match lit.node {
        LitStr(..) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use ast::*;
    use super::*;

    fn ident_to_segment(id : &Ident) -> PathSegment {
        PathSegment {identifier: id.clone(),
                     parameters: PathParameters::none()}
    }

    #[test] fn idents_name_eq_test() {
        assert!(segments_name_eq(
            &[Ident{name:Name(3),ctxt:4}, Ident{name:Name(78),ctxt:82}]
                .iter().map(ident_to_segment).collect::<Vec<PathSegment>>(),
            &[Ident{name:Name(3),ctxt:104}, Ident{name:Name(78),ctxt:182}]
                .iter().map(ident_to_segment).collect::<Vec<PathSegment>>()));
        assert!(!segments_name_eq(
            &[Ident{name:Name(3),ctxt:4}, Ident{name:Name(78),ctxt:82}]
                .iter().map(ident_to_segment).collect::<Vec<PathSegment>>(),
            &[Ident{name:Name(3),ctxt:104}, Ident{name:Name(77),ctxt:182}]
                .iter().map(ident_to_segment).collect::<Vec<PathSegment>>()));
    }
}
