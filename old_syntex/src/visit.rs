// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! AST walker. Each overridden visit method has full control over what
//! happens with its node, it can do its own traversal of the node's children,
//! call `visit::walk_*` to apply the default traversal algorithm, or prevent
//! deeper traversal by doing nothing.
//!
//! Note: it is an important invariant that the default visitor walks the body
//! of a function in "execution order" (more concretely, reverse post-order
//! with respect to the CFG implied by the AST), meaning that if AST node A may
//! execute before AST node B, then A is visited first.  The borrow checker in
//! particular relies on this property.
//!
//! Note: walking an AST before macro expansion is probably a bad idea. For
//! instance, a walker looking for item names in a module will miss all of
//! those that are created by the expansion of a macro.

pub use self::FnKind::*;

use abi::Abi;
use ast::*;
use ast;
use codemap::Span;
use ptr::P;
use owned_slice::OwnedSlice;

#[derive(Copy, Clone)]
pub enum FnKind<'a> {
    /// fn foo() or extern "Abi" fn foo()
    FkItemFn(Ident, &'a Generics, Unsafety, Constness, Abi, Visibility),

    /// fn foo(&self)
    FkMethod(Ident, &'a MethodSig, Option<Visibility>),

    /// |x, y| ...
    /// proc(x, y) ...
    FkFnBlock,
}

/// Each method of the Visitor trait is a hook to be potentially
/// overridden.  Each method's default implementation recursively visits
/// the substructure of the input via the corresponding `walk` method;
/// e.g. the `visit_mod` method by default calls `visit::walk_mod`.
///
/// If you want to ensure that your code handles every variant
/// explicitly, you need to override each method.  (And you also need
/// to monitor future changes to `Visitor` in case a new method with a
/// new default implementation gets introduced.)
pub trait Visitor<'v> : Sized {
    fn visit_name(&mut self, _span: Span, _name: Name) {
        // Nothing to do.
    }
    fn visit_ident(&mut self, span: Span, ident: Ident) {
        self.visit_name(span, ident.name);
    }
    fn visit_mod(&mut self, m: &'v Mod, _s: Span, _n: NodeId) { walk_mod(self, m) }
    fn visit_foreign_item(&mut self, i: &'v ForeignItem) { walk_foreign_item(self, i) }
    fn visit_item(&mut self, i: &'v Item) { walk_item(self, i) }
    fn visit_local(&mut self, l: &'v Local) { walk_local(self, l) }
    fn visit_block(&mut self, b: &'v Block) { walk_block(self, b) }
    fn visit_stmt(&mut self, s: &'v Stmt) { walk_stmt(self, s) }
    fn visit_arm(&mut self, a: &'v Arm) { walk_arm(self, a) }
    fn visit_pat(&mut self, p: &'v Pat) { walk_pat(self, p) }
    fn visit_decl(&mut self, d: &'v Decl) { walk_decl(self, d) }
    fn visit_expr(&mut self, ex: &'v Expr) { walk_expr(self, ex) }
    fn visit_expr_post(&mut self, _ex: &'v Expr) { }
    fn visit_ty(&mut self, t: &'v Ty) { walk_ty(self, t) }
    fn visit_generics(&mut self, g: &'v Generics) { walk_generics(self, g) }
    fn visit_fn(&mut self, fk: FnKind<'v>, fd: &'v FnDecl, b: &'v Block, s: Span, _: NodeId) {
        walk_fn(self, fk, fd, b, s)
    }
    fn visit_trait_item(&mut self, ti: &'v TraitItem) { walk_trait_item(self, ti) }
    fn visit_impl_item(&mut self, ii: &'v ImplItem) { walk_impl_item(self, ii) }
    fn visit_trait_ref(&mut self, t: &'v TraitRef) { walk_trait_ref(self, t) }
    fn visit_ty_param_bound(&mut self, bounds: &'v TyParamBound) {
        walk_ty_param_bound(self, bounds)
    }
    fn visit_poly_trait_ref(&mut self, t: &'v PolyTraitRef, m: &'v TraitBoundModifier) {
        walk_poly_trait_ref(self, t, m)
    }
    fn visit_struct_def(&mut self, s: &'v StructDef, _: Ident, _: &'v Generics, _: NodeId) {
        walk_struct_def(self, s)
    }
    fn visit_struct_field(&mut self, s: &'v StructField) { walk_struct_field(self, s) }
    fn visit_variant(&mut self, v: &'v Variant, g: &'v Generics) { walk_variant(self, v, g) }

    /// Visits an optional reference to a lifetime. The `span` is the span of some surrounding
    /// reference should opt_lifetime be None.
    fn visit_opt_lifetime_ref(&mut self,
                              _span: Span,
                              opt_lifetime: &'v Option<Lifetime>) {
        match *opt_lifetime {
            Some(ref l) => self.visit_lifetime_ref(l),
            None => ()
        }
    }
    fn visit_lifetime_bound(&mut self, lifetime: &'v Lifetime) {
        walk_lifetime_bound(self, lifetime)
    }
    fn visit_lifetime_ref(&mut self, lifetime: &'v Lifetime) {
        walk_lifetime_ref(self, lifetime)
    }
    fn visit_lifetime_def(&mut self, lifetime: &'v LifetimeDef) {
        walk_lifetime_def(self, lifetime)
    }
    fn visit_explicit_self(&mut self, es: &'v ExplicitSelf) {
        walk_explicit_self(self, es)
    }
    fn visit_mac(&mut self, _mac: &'v Mac) {
        panic!("visit_mac disabled by default");
        // NB: see note about macros above.
        // if you really want a visitor that
        // works on macros, use this
        // definition in your trait impl:
        // visit::walk_mac(self, _mac)
    }
    fn visit_path(&mut self, path: &'v Path, _id: ast::NodeId) {
        walk_path(self, path)
    }
    fn visit_path_segment(&mut self, path_span: Span, path_segment: &'v PathSegment) {
        walk_path_segment(self, path_span, path_segment)
    }
    fn visit_path_parameters(&mut self, path_span: Span, path_parameters: &'v PathParameters) {
        walk_path_parameters(self, path_span, path_parameters)
    }
    fn visit_assoc_type_binding(&mut self, type_binding: &'v TypeBinding) {
        walk_assoc_type_binding(self, type_binding)
    }
    fn visit_attribute(&mut self, _attr: &'v Attribute) {}
}

pub fn walk_inlined_item<'v,V>(visitor: &mut V, item: &'v InlinedItem)
                         where V: Visitor<'v> {
    match *item {
        IIItem(ref i) => visitor.visit_item(&**i),
        IIForeign(ref i) => visitor.visit_foreign_item(&**i),
        IITraitItem(_, ref ti) => visitor.visit_trait_item(ti),
        IIImplItem(_, ref ii) => visitor.visit_impl_item(ii),
    }
}


pub fn walk_crate<'v, V: Visitor<'v>>(visitor: &mut V, krate: &'v Crate) {
    visitor.visit_mod(&krate.module, krate.span, CRATE_NODE_ID);
    for attr in &krate.attrs {
        visitor.visit_attribute(attr);
    }
}

pub fn walk_mod<'v, V: Visitor<'v>>(visitor: &mut V, module: &'v Mod) {
    for item in &module.items {
        visitor.visit_item(&**item)
    }
}

pub fn walk_local<'v, V: Visitor<'v>>(visitor: &mut V, local: &'v Local) {
    visitor.visit_pat(&*local.pat);
    walk_ty_opt(visitor, &local.ty);
    walk_expr_opt(visitor, &local.init);
}

pub fn walk_lifetime_def<'v, V: Visitor<'v>>(visitor: &mut V,
                                              lifetime_def: &'v LifetimeDef) {
    visitor.visit_name(lifetime_def.lifetime.span, lifetime_def.lifetime.name);
    for bound in &lifetime_def.bounds {
        visitor.visit_lifetime_bound(bound);
    }
}

pub fn walk_lifetime_bound<'v, V: Visitor<'v>>(visitor: &mut V,
                                               lifetime_ref: &'v Lifetime) {
    visitor.visit_lifetime_ref(lifetime_ref)
}

pub fn walk_lifetime_ref<'v, V: Visitor<'v>>(visitor: &mut V,
                                             lifetime_ref: &'v Lifetime) {
    visitor.visit_name(lifetime_ref.span, lifetime_ref.name)
}

pub fn walk_explicit_self<'v, V: Visitor<'v>>(visitor: &mut V,
                                              explicit_self: &'v ExplicitSelf) {
    match explicit_self.node {
        SelfStatic | SelfValue(_) => {},
        SelfRegion(ref lifetime, _, _) => {
            visitor.visit_opt_lifetime_ref(explicit_self.span, lifetime)
        }
        SelfExplicit(ref typ, _) => visitor.visit_ty(&**typ),
    }
}

pub fn walk_poly_trait_ref<'v, V>(visitor: &mut V,
                                  trait_ref: &'v PolyTraitRef,
                                  _modifier: &'v TraitBoundModifier)
    where V: Visitor<'v>
{
    walk_lifetime_decls_helper(visitor, &trait_ref.bound_lifetimes);
    visitor.visit_trait_ref(&trait_ref.trait_ref);
}

pub fn walk_trait_ref<'v,V>(visitor: &mut V,
                                   trait_ref: &'v TraitRef)
    where V: Visitor<'v>
{
    visitor.visit_path(&trait_ref.path, trait_ref.ref_id)
}

pub fn walk_item<'v, V: Visitor<'v>>(visitor: &mut V, item: &'v Item) {
    visitor.visit_ident(item.span, item.ident);
    match item.node {
        ItemExternCrate(..) => {}
        ItemUse(ref vp) => {
            match vp.node {
                ViewPathSimple(ident, ref path) => {
                    visitor.visit_ident(vp.span, ident);
                    visitor.visit_path(path, item.id);
                }
                ViewPathGlob(ref path) => {
                    visitor.visit_path(path, item.id);
                }
                ViewPathList(ref prefix, ref list) => {
                    for id in list {
                        match id.node {
                            PathListIdent { name, .. } => {
                                visitor.visit_ident(id.span, name);
                            }
                            PathListMod { .. } => ()
                        }
                    }

                    // Note that the `prefix` here is not a complete
                    // path, so we don't use `visit_path`.
                    walk_path(visitor, prefix);
                }
            }
        }
        ItemStatic(ref typ, _, ref expr) |
        ItemConst(ref typ, ref expr) => {
            visitor.visit_ty(&**typ);
            visitor.visit_expr(&**expr);
        }
        ItemFn(ref declaration, unsafety, constness, abi, ref generics, ref body) => {
            visitor.visit_fn(FkItemFn(item.ident, generics, unsafety,
                                      constness, abi, item.vis),
                             &**declaration,
                             &**body,
                             item.span,
                             item.id)
        }
        ItemMod(ref module) => {
            visitor.visit_mod(module, item.span, item.id)
        }
        ItemForeignMod(ref foreign_module) => {
            for foreign_item in &foreign_module.items {
                visitor.visit_foreign_item(&**foreign_item)
            }
        }
        ItemTy(ref typ, ref type_parameters) => {
            visitor.visit_ty(&**typ);
            visitor.visit_generics(type_parameters)
        }
        ItemEnum(ref enum_definition, ref type_parameters) => {
            visitor.visit_generics(type_parameters);
            walk_enum_def(visitor, enum_definition, type_parameters)
        }
        ItemDefaultImpl(_, ref trait_ref) => {
            visitor.visit_trait_ref(trait_ref)
        }
        ItemImpl(_, _,
                 ref type_parameters,
                 ref trait_reference,
                 ref typ,
                 ref impl_items) => {
            visitor.visit_generics(type_parameters);
            match *trait_reference {
                Some(ref trait_reference) => visitor.visit_trait_ref(trait_reference),
                None => ()
            }
            visitor.visit_ty(&**typ);
            for impl_item in impl_items {
                visitor.visit_impl_item(impl_item);
            }
        }
        ItemStruct(ref struct_definition, ref generics) => {
            visitor.visit_generics(generics);
            visitor.visit_struct_def(&**struct_definition,
                                     item.ident,
                                     generics,
                                     item.id)
        }
        ItemTrait(_, ref generics, ref bounds, ref methods) => {
            visitor.visit_generics(generics);
            walk_ty_param_bounds_helper(visitor, bounds);
            for method in methods {
                visitor.visit_trait_item(method)
            }
        }
        ItemMac(ref mac) => visitor.visit_mac(mac),
    }
    for attr in &item.attrs {
        visitor.visit_attribute(attr);
    }
}

pub fn walk_enum_def<'v, V: Visitor<'v>>(visitor: &mut V,
                                         enum_definition: &'v EnumDef,
                                         generics: &'v Generics) {
    for variant in &enum_definition.variants {
        visitor.visit_variant(&**variant, generics);
    }
}

pub fn walk_variant<'v, V: Visitor<'v>>(visitor: &mut V,
                                        variant: &'v Variant,
                                        generics: &'v Generics) {
    visitor.visit_ident(variant.span, variant.node.name);

    match variant.node.kind {
        TupleVariantKind(ref variant_arguments) => {
            for variant_argument in variant_arguments {
                visitor.visit_ty(&*variant_argument.ty)
            }
        }
        StructVariantKind(ref struct_definition) => {
            visitor.visit_struct_def(&**struct_definition,
                                     variant.node.name,
                                     generics,
                                     variant.node.id)
        }
    }
    match variant.node.disr_expr {
        Some(ref expr) => visitor.visit_expr(&**expr),
        None => ()
    }
    for attr in &variant.node.attrs {
        visitor.visit_attribute(attr);
    }
}

pub fn skip_ty<'v, V: Visitor<'v>>(_: &mut V, _: &'v Ty) {
    // Empty!
}

pub fn walk_ty_opt<'v, V: Visitor<'v>>(visitor: &mut V, optional_type: &'v Option<P<Ty>>) {
    match *optional_type {
        Some(ref ty) => visitor.visit_ty(&**ty),
        None => ()
    }
}

pub fn walk_ty<'v, V: Visitor<'v>>(visitor: &mut V, typ: &'v Ty) {
    match typ.node {
        TyVec(ref ty) | TyParen(ref ty) => {
            visitor.visit_ty(&**ty)
        }
        TyPtr(ref mutable_type) => {
            visitor.visit_ty(&*mutable_type.ty)
        }
        TyRptr(ref lifetime, ref mutable_type) => {
            visitor.visit_opt_lifetime_ref(typ.span, lifetime);
            visitor.visit_ty(&*mutable_type.ty)
        }
        TyTup(ref tuple_element_types) => {
            for tuple_element_type in tuple_element_types {
                visitor.visit_ty(&**tuple_element_type)
            }
        }
        TyBareFn(ref function_declaration) => {
            for argument in &function_declaration.decl.inputs {
                visitor.visit_ty(&*argument.ty)
            }
            walk_fn_ret_ty(visitor, &function_declaration.decl.output);
            walk_lifetime_decls_helper(visitor, &function_declaration.lifetimes);
        }
        TyPath(ref maybe_qself, ref path) => {
            if let Some(ref qself) = *maybe_qself {
                visitor.visit_ty(&qself.ty);
            }
            visitor.visit_path(path, typ.id);
        }
        TyObjectSum(ref ty, ref bounds) => {
            visitor.visit_ty(&**ty);
            walk_ty_param_bounds_helper(visitor, bounds);
        }
        TyFixedLengthVec(ref ty, ref expression) => {
            visitor.visit_ty(&**ty);
            visitor.visit_expr(&**expression)
        }
        TyPolyTraitRef(ref bounds) => {
            walk_ty_param_bounds_helper(visitor, bounds)
        }
        TyTypeof(ref expression) => {
            visitor.visit_expr(&**expression)
        }
        TyInfer => {}
    }
}

pub fn walk_lifetime_decls_helper<'v, V: Visitor<'v>>(visitor: &mut V,
                                                      lifetimes: &'v Vec<LifetimeDef>) {
    for l in lifetimes {
        visitor.visit_lifetime_def(l);
    }
}

pub fn walk_path<'v, V: Visitor<'v>>(visitor: &mut V, path: &'v Path) {
    for segment in &path.segments {
        visitor.visit_path_segment(path.span, segment);
    }
}

pub fn walk_path_segment<'v, V: Visitor<'v>>(visitor: &mut V,
                                             path_span: Span,
                                             segment: &'v PathSegment) {
    visitor.visit_ident(path_span, segment.identifier);
    visitor.visit_path_parameters(path_span, &segment.parameters);
}

pub fn walk_path_parameters<'v, V: Visitor<'v>>(visitor: &mut V,
                                                _path_span: Span,
                                                path_parameters: &'v PathParameters) {
    match *path_parameters {
        ast::AngleBracketedParameters(ref data) => {
            for typ in &*data.types {
                visitor.visit_ty(&**typ);
            }
            for lifetime in &data.lifetimes {
                visitor.visit_lifetime_ref(lifetime);
            }
            for binding in &*data.bindings {
                visitor.visit_assoc_type_binding(&**binding);
            }
        }
        ast::ParenthesizedParameters(ref data) => {
            for typ in &data.inputs {
                visitor.visit_ty(&**typ);
            }
            if let Some(ref typ) = data.output {
                visitor.visit_ty(&**typ);
            }
        }
    }
}

pub fn walk_assoc_type_binding<'v, V: Visitor<'v>>(visitor: &mut V,
                                                   type_binding: &'v TypeBinding) {
    visitor.visit_ident(type_binding.span, type_binding.ident);
    visitor.visit_ty(&*type_binding.ty);
}

pub fn walk_pat<'v, V: Visitor<'v>>(visitor: &mut V, pattern: &'v Pat) {
    match pattern.node {
        PatEnum(ref path, ref children) => {
            visitor.visit_path(path, pattern.id);
            if let Some(ref children) = *children {
                for child in children {
                    visitor.visit_pat(&*child)
                }
            }
        }
        PatQPath(ref qself, ref path) => {
            visitor.visit_ty(&qself.ty);
            visitor.visit_path(path, pattern.id)
        }
        PatStruct(ref path, ref fields, _) => {
            visitor.visit_path(path, pattern.id);
            for field in fields {
                visitor.visit_pat(&*field.node.pat)
            }
        }
        PatTup(ref tuple_elements) => {
            for tuple_element in tuple_elements {
                visitor.visit_pat(&**tuple_element)
            }
        }
        PatBox(ref subpattern) |
        PatRegion(ref subpattern, _) => {
            visitor.visit_pat(&**subpattern)
        }
        PatIdent(_, ref pth1, ref optional_subpattern) => {
            visitor.visit_ident(pth1.span, pth1.node);
            match *optional_subpattern {
                None => {}
                Some(ref subpattern) => visitor.visit_pat(&**subpattern),
            }
        }
        PatLit(ref expression) => visitor.visit_expr(&**expression),
        PatRange(ref lower_bound, ref upper_bound) => {
            visitor.visit_expr(&**lower_bound);
            visitor.visit_expr(&**upper_bound)
        }
        PatWild(_) => (),
        PatVec(ref prepattern, ref slice_pattern, ref postpatterns) => {
            for prepattern in prepattern {
                visitor.visit_pat(&**prepattern)
            }
            if let Some(ref slice_pattern) = *slice_pattern {
                visitor.visit_pat(&**slice_pattern)
            }
            for postpattern in postpatterns {
                visitor.visit_pat(&**postpattern)
            }
        }
        PatMac(ref mac) => visitor.visit_mac(mac),
    }
}

pub fn walk_foreign_item<'v, V: Visitor<'v>>(visitor: &mut V,
                                             foreign_item: &'v ForeignItem) {
    visitor.visit_ident(foreign_item.span, foreign_item.ident);

    match foreign_item.node {
        ForeignItemFn(ref function_declaration, ref generics) => {
            walk_fn_decl(visitor, &**function_declaration);
            visitor.visit_generics(generics)
        }
        ForeignItemStatic(ref typ, _) => visitor.visit_ty(&**typ),
    }

    for attr in &foreign_item.attrs {
        visitor.visit_attribute(attr);
    }
}

pub fn walk_ty_param_bounds_helper<'v, V: Visitor<'v>>(visitor: &mut V,
                                                       bounds: &'v OwnedSlice<TyParamBound>) {
    for bound in &**bounds {
        visitor.visit_ty_param_bound(bound)
    }
}

pub fn walk_ty_param_bound<'v, V: Visitor<'v>>(visitor: &mut V,
                                               bound: &'v TyParamBound) {
    match *bound {
        TraitTyParamBound(ref typ, ref modifier) => {
            visitor.visit_poly_trait_ref(typ, modifier);
        }
        RegionTyParamBound(ref lifetime) => {
            visitor.visit_lifetime_bound(lifetime);
        }
    }
}

pub fn walk_generics<'v, V: Visitor<'v>>(visitor: &mut V, generics: &'v Generics) {
    for param in &*generics.ty_params {
        visitor.visit_ident(param.span, param.ident);
        walk_ty_param_bounds_helper(visitor, &param.bounds);
        walk_ty_opt(visitor, &param.default);
    }
    walk_lifetime_decls_helper(visitor, &generics.lifetimes);
    for predicate in &generics.where_clause.predicates {
        match predicate {
            &ast::WherePredicate::BoundPredicate(ast::WhereBoundPredicate{ref bounded_ty,
                                                                          ref bounds,
                                                                          ..}) => {
                visitor.visit_ty(&**bounded_ty);
                walk_ty_param_bounds_helper(visitor, bounds);
            }
            &ast::WherePredicate::RegionPredicate(ast::WhereRegionPredicate{ref lifetime,
                                                                            ref bounds,
                                                                            ..}) => {
                visitor.visit_lifetime_ref(lifetime);

                for bound in bounds {
                    visitor.visit_lifetime_ref(bound);
                }
            }
            &ast::WherePredicate::EqPredicate(ast::WhereEqPredicate{id,
                                                                    ref path,
                                                                    ref ty,
                                                                    ..}) => {
                visitor.visit_path(path, id);
                visitor.visit_ty(&**ty);
            }
        }
    }
}

pub fn walk_fn_ret_ty<'v, V: Visitor<'v>>(visitor: &mut V, ret_ty: &'v FunctionRetTy) {
    if let Return(ref output_ty) = *ret_ty {
        visitor.visit_ty(&**output_ty)
    }
}

pub fn walk_fn_decl<'v, V: Visitor<'v>>(visitor: &mut V, function_declaration: &'v FnDecl) {
    for argument in &function_declaration.inputs {
        visitor.visit_pat(&*argument.pat);
        visitor.visit_ty(&*argument.ty)
    }
    walk_fn_ret_ty(visitor, &function_declaration.output)
}

pub fn walk_fn<'v, V: Visitor<'v>>(visitor: &mut V,
                                   function_kind: FnKind<'v>,
                                   function_declaration: &'v FnDecl,
                                   function_body: &'v Block,
                                   _span: Span) {
    walk_fn_decl(visitor, function_declaration);

    match function_kind {
        FkItemFn(_, generics, _, _, _, _) => {
            visitor.visit_generics(generics);
        }
        FkMethod(_, sig, _) => {
            visitor.visit_generics(&sig.generics);
            visitor.visit_explicit_self(&sig.explicit_self);
        }
        FkFnBlock => {}
    }

    visitor.visit_block(function_body)
}

pub fn walk_trait_item<'v, V: Visitor<'v>>(visitor: &mut V, trait_item: &'v TraitItem) {
    visitor.visit_ident(trait_item.span, trait_item.ident);
    for attr in &trait_item.attrs {
        visitor.visit_attribute(attr);
    }
    match trait_item.node {
        ConstTraitItem(ref ty, ref default) => {
            visitor.visit_ty(ty);
            if let Some(ref expr) = *default {
                visitor.visit_expr(expr);
            }
        }
        MethodTraitItem(ref sig, None) => {
            visitor.visit_explicit_self(&sig.explicit_self);
            visitor.visit_generics(&sig.generics);
            walk_fn_decl(visitor, &sig.decl);
        }
        MethodTraitItem(ref sig, Some(ref body)) => {
            visitor.visit_fn(FkMethod(trait_item.ident, sig, None), &sig.decl,
                             body, trait_item.span, trait_item.id);
        }
        TypeTraitItem(ref bounds, ref default) => {
            walk_ty_param_bounds_helper(visitor, bounds);
            walk_ty_opt(visitor, default);
        }
    }
}

pub fn walk_impl_item<'v, V: Visitor<'v>>(visitor: &mut V, impl_item: &'v ImplItem) {
    visitor.visit_ident(impl_item.span, impl_item.ident);
    for attr in &impl_item.attrs {
        visitor.visit_attribute(attr);
    }
    match impl_item.node {
        ConstImplItem(ref ty, ref expr) => {
            visitor.visit_ty(ty);
            visitor.visit_expr(expr);
        }
        MethodImplItem(ref sig, ref body) => {
            visitor.visit_fn(FkMethod(impl_item.ident, sig, Some(impl_item.vis)), &sig.decl,
                             body, impl_item.span, impl_item.id);
        }
        TypeImplItem(ref ty) => {
            visitor.visit_ty(ty);
        }
        MacImplItem(ref mac) => {
            visitor.visit_mac(mac);
        }
    }
}

pub fn walk_struct_def<'v, V: Visitor<'v>>(visitor: &mut V,
                                           struct_definition: &'v StructDef) {
    for field in &struct_definition.fields {
        visitor.visit_struct_field(field)
    }
}

pub fn walk_struct_field<'v, V: Visitor<'v>>(visitor: &mut V,
                                             struct_field: &'v StructField) {
    if let NamedField(name, _) = struct_field.node.kind {
        visitor.visit_ident(struct_field.span, name);
    }

    visitor.visit_ty(&*struct_field.node.ty);

    for attr in &struct_field.node.attrs {
        visitor.visit_attribute(attr);
    }
}

pub fn walk_block<'v, V: Visitor<'v>>(visitor: &mut V, block: &'v Block) {
    for statement in &block.stmts {
        visitor.visit_stmt(&**statement)
    }
    walk_expr_opt(visitor, &block.expr)
}

pub fn walk_stmt<'v, V: Visitor<'v>>(visitor: &mut V, statement: &'v Stmt) {
    match statement.node {
        StmtDecl(ref declaration, _) => visitor.visit_decl(&**declaration),
        StmtExpr(ref expression, _) | StmtSemi(ref expression, _) => {
            visitor.visit_expr(&**expression)
        }
        StmtMac(ref mac, _) => visitor.visit_mac(&**mac),
    }
}

pub fn walk_decl<'v, V: Visitor<'v>>(visitor: &mut V, declaration: &'v Decl) {
    match declaration.node {
        DeclLocal(ref local) => visitor.visit_local(&**local),
        DeclItem(ref item) => visitor.visit_item(&**item),
    }
}

pub fn walk_expr_opt<'v, V: Visitor<'v>>(visitor: &mut V,
                                         optional_expression: &'v Option<P<Expr>>) {
    match *optional_expression {
        None => {}
        Some(ref expression) => visitor.visit_expr(&**expression),
    }
}

pub fn walk_exprs<'v, V: Visitor<'v>>(visitor: &mut V, expressions: &'v [P<Expr>]) {
    for expression in expressions {
        visitor.visit_expr(&**expression)
    }
}

pub fn walk_mac<'v, V: Visitor<'v>>(_: &mut V, _: &'v Mac) {
    // Empty!
}

pub fn walk_expr<'v, V: Visitor<'v>>(visitor: &mut V, expression: &'v Expr) {
    match expression.node {
        ExprBox(ref place, ref subexpression) => {
            place.as_ref().map(|e|visitor.visit_expr(&**e));
            visitor.visit_expr(&**subexpression)
        }
        ExprVec(ref subexpressions) => {
            walk_exprs(visitor, subexpressions)
        }
        ExprRepeat(ref element, ref count) => {
            visitor.visit_expr(&**element);
            visitor.visit_expr(&**count)
        }
        ExprStruct(ref path, ref fields, ref optional_base) => {
            visitor.visit_path(path, expression.id);
            for field in fields {
                visitor.visit_expr(&*field.expr)
            }
            walk_expr_opt(visitor, optional_base)
        }
        ExprTup(ref subexpressions) => {
            for subexpression in subexpressions {
                visitor.visit_expr(&**subexpression)
            }
        }
        ExprCall(ref callee_expression, ref arguments) => {
            for argument in arguments {
                visitor.visit_expr(&**argument)
            }
            visitor.visit_expr(&**callee_expression)
        }
        ExprMethodCall(_, ref types, ref arguments) => {
            walk_exprs(visitor, arguments);
            for typ in types {
                visitor.visit_ty(&**typ)
            }
        }
        ExprBinary(_, ref left_expression, ref right_expression) => {
            visitor.visit_expr(&**left_expression);
            visitor.visit_expr(&**right_expression)
        }
        ExprAddrOf(_, ref subexpression) | ExprUnary(_, ref subexpression) => {
            visitor.visit_expr(&**subexpression)
        }
        ExprLit(_) => {}
        ExprCast(ref subexpression, ref typ) => {
            visitor.visit_expr(&**subexpression);
            visitor.visit_ty(&**typ)
        }
        ExprIf(ref head_expression, ref if_block, ref optional_else) => {
            visitor.visit_expr(&**head_expression);
            visitor.visit_block(&**if_block);
            walk_expr_opt(visitor, optional_else)
        }
        ExprWhile(ref subexpression, ref block, _) => {
            visitor.visit_expr(&**subexpression);
            visitor.visit_block(&**block)
        }
        ExprIfLet(ref pattern, ref subexpression, ref if_block, ref optional_else) => {
            visitor.visit_pat(&**pattern);
            visitor.visit_expr(&**subexpression);
            visitor.visit_block(&**if_block);
            walk_expr_opt(visitor, optional_else);
        }
        ExprWhileLet(ref pattern, ref subexpression, ref block, _) => {
            visitor.visit_pat(&**pattern);
            visitor.visit_expr(&**subexpression);
            visitor.visit_block(&**block);
        }
        ExprForLoop(ref pattern, ref subexpression, ref block, _) => {
            visitor.visit_pat(&**pattern);
            visitor.visit_expr(&**subexpression);
            visitor.visit_block(&**block)
        }
        ExprLoop(ref block, _) => visitor.visit_block(&**block),
        ExprMatch(ref subexpression, ref arms, _) => {
            visitor.visit_expr(&**subexpression);
            for arm in arms {
                visitor.visit_arm(arm)
            }
        }
        ExprClosure(_, ref function_declaration, ref body) => {
            visitor.visit_fn(FkFnBlock,
                             &**function_declaration,
                             &**body,
                             expression.span,
                             expression.id)
        }
        ExprBlock(ref block) => visitor.visit_block(&**block),
        ExprAssign(ref left_hand_expression, ref right_hand_expression) => {
            visitor.visit_expr(&**right_hand_expression);
            visitor.visit_expr(&**left_hand_expression)
        }
        ExprAssignOp(_, ref left_expression, ref right_expression) => {
            visitor.visit_expr(&**right_expression);
            visitor.visit_expr(&**left_expression)
        }
        ExprField(ref subexpression, _) => {
            visitor.visit_expr(&**subexpression);
        }
        ExprTupField(ref subexpression, _) => {
            visitor.visit_expr(&**subexpression);
        }
        ExprIndex(ref main_expression, ref index_expression) => {
            visitor.visit_expr(&**main_expression);
            visitor.visit_expr(&**index_expression)
        }
        ExprRange(ref start, ref end) => {
            walk_expr_opt(visitor, start);
            walk_expr_opt(visitor, end)
        }
        ExprPath(ref maybe_qself, ref path) => {
            if let Some(ref qself) = *maybe_qself {
                visitor.visit_ty(&qself.ty);
            }
            visitor.visit_path(path, expression.id)
        }
        ExprBreak(_) | ExprAgain(_) => {}
        ExprRet(ref optional_expression) => {
            walk_expr_opt(visitor, optional_expression)
        }
        ExprMac(ref mac, _) => visitor.visit_mac(mac),
        ExprParen(ref subexpression) => {
            visitor.visit_expr(&**subexpression)
        }
        ExprInlineAsm(ref ia) => {
            for input in &ia.inputs {
                let (_, ref input) = *input;
                visitor.visit_expr(&**input)
            }
            for output in &ia.outputs {
                let (_, ref output, _) = *output;
                visitor.visit_expr(&**output)
            }
        }
    }

    visitor.visit_expr_post(expression)
}

pub fn walk_arm<'v, V: Visitor<'v>>(visitor: &mut V, arm: &'v Arm) {
    for pattern in &arm.pats {
        visitor.visit_pat(&**pattern)
    }
    walk_expr_opt(visitor, &arm.guard);
    visitor.visit_expr(&*arm.body);
    for attr in &arm.attrs {
        visitor.visit_attribute(attr);
    }
}
