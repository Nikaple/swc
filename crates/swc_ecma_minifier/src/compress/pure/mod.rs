#![allow(clippy::needless_update)]

use std::sync::Arc;

use rayon::prelude::*;
use swc_common::{collections::AHashSet, pass::Repeated, util::take::Take, DUMMY_SP, GLOBALS};
use swc_ecma_ast::*;
use swc_ecma_utils::collect_decls;
use swc_ecma_visit::{noop_visit_mut_type, VisitMut, VisitMutWith, VisitWith};
use tracing::{span, Level};

use self::{ctx::Ctx, misc::DropOpts};
use crate::{
    debug::{dump, AssertValid},
    marks::Marks,
    option::CompressOptions,
    util::ModuleItemExt,
    MAX_PAR_DEPTH,
};

mod arrows;
mod bools;
mod conds;
mod ctx;
mod dead_code;
mod drop_console;
mod evaluate;
mod if_return;
mod loops;
mod misc;
mod numbers;
mod properties;
mod sequences;
mod strings;
mod unsafes;
mod vars;

#[allow(clippy::needless_lifetimes)]
pub(crate) fn pure_optimizer<'a>(
    options: &'a CompressOptions,
    marks: Marks,
    force_str_for_tpl: bool,
    enable_everything: bool,
    debug_infinite_loop: bool,
) -> impl 'a + VisitMut + Repeated {
    Pure {
        options,
        marks,
        ctx: Ctx {
            force_str_for_tpl,
            ..Default::default()
        },
        changed: Default::default(),
        enable_everything,
        debug_infinite_loop,
        bindings: Default::default(),
    }
}

struct Pure<'a> {
    options: &'a CompressOptions,
    marks: Marks,
    ctx: Ctx,
    changed: bool,
    enable_everything: bool,

    debug_infinite_loop: bool,

    bindings: Option<Arc<AHashSet<Id>>>,
}

impl Repeated for Pure<'_> {
    fn changed(&self) -> bool {
        self.changed
    }

    fn reset(&mut self) {
        self.bindings = None;
        self.ctx = Default::default();
        self.changed = false;
    }
}

impl Pure<'_> {
    fn handle_stmt_likes<T>(&mut self, stmts: &mut Vec<T>)
    where
        T: ModuleItemExt + Take,
        Vec<T>: VisitWith<self::vars::VarWithOutInitCounter>
            + VisitMutWith<self::vars::VarPrepender>
            + VisitMutWith<self::vars::VarMover>
            + VisitWith<AssertValid>,
    {
        self.remove_dead_branch(stmts);

        if cfg!(debug_assertions) {
            stmts.visit_with(&mut AssertValid);
        }

        self.drop_unreachable_stmts(stmts);

        if cfg!(debug_assertions) {
            stmts.visit_with(&mut AssertValid);
        }

        self.drop_useless_blocks(stmts);

        if cfg!(debug_assertions) {
            stmts.visit_with(&mut AssertValid);
        }

        self.collapse_vars_without_init(stmts);

        if cfg!(debug_assertions) {
            stmts.visit_with(&mut AssertValid);
        }

        if self.enable_everything {
            self.join_vars(stmts);

            if cfg!(debug_assertions) {
                stmts.visit_with(&mut AssertValid);
            }
        }

        stmts.retain(|s| !matches!(s.as_stmt(), Some(Stmt::Empty(..))));
    }

    fn optimize_fn_stmts(&mut self, stmts: &mut Vec<Stmt>) {
        if !stmts.is_empty() {
            if let Stmt::Expr(ExprStmt { expr, .. }) = &stmts[0] {
                if let Expr::Lit(Lit::Str(v)) = &**expr {
                    if v.value == *"use asm" {
                        return;
                    }
                }
            }
        }

        self.remove_useless_return(stmts);

        self.negate_if_terminate(stmts, true, false);

        if let Some(last) = stmts.last_mut() {
            self.drop_unused_stmt_at_end_of_fn(last);
        }
    }

    /// Visit `nodes`, maybe in parallel.
    fn visit_par<N>(&mut self, nodes: &mut Vec<N>)
    where
        N: for<'aa> VisitMutWith<Pure<'aa>> + Send + Sync,
    {
        if self.ctx.par_depth >= MAX_PAR_DEPTH * 2 || cfg!(target_arch = "wasm32") {
            for node in nodes {
                let mut v = Pure {
                    options: self.options,
                    marks: self.marks,
                    ctx: self.ctx,
                    changed: false,
                    enable_everything: self.enable_everything,
                    debug_infinite_loop: self.debug_infinite_loop,
                    bindings: self.bindings.clone(),
                };
                node.visit_mut_with(&mut v);

                self.changed |= v.changed;
            }
        } else {
            GLOBALS.with(|globals| {
                let changed = nodes
                    .par_iter_mut()
                    .map(|node| {
                        GLOBALS.set(globals, || {
                            let mut v = Pure {
                                options: self.options,
                                marks: self.marks,
                                ctx: Ctx {
                                    par_depth: self.ctx.par_depth + 1,
                                    ..self.ctx
                                },
                                changed: false,
                                enable_everything: self.enable_everything,
                                debug_infinite_loop: self.debug_infinite_loop,
                                bindings: self.bindings.clone(),
                            };
                            node.visit_mut_with(&mut v);

                            v.changed
                        })
                    })
                    .reduce(|| false, |a, b| a || b);

                self.changed |= changed;
            });
        }
    }
}

impl VisitMut for Pure<'_> {
    noop_visit_mut_type!();

    fn visit_mut_assign_expr(&mut self, e: &mut AssignExpr) {
        {
            let ctx = Ctx {
                is_lhs_of_assign: true,
                ..self.ctx
            };
            e.left.visit_mut_children_with(&mut *self.with_ctx(ctx));
        }

        e.right.visit_mut_with(self);
    }

    fn visit_mut_bin_expr(&mut self, e: &mut BinExpr) {
        e.visit_mut_children_with(self);

        self.compress_cmp_with_long_op(e);

        self.optimize_cmp_with_null_or_undefined(e);

        if e.op == op!(bin, "+") {
            self.concat_tpl(&mut e.left, &mut e.right);
        }
    }

    fn visit_mut_block_stmt_or_expr(&mut self, body: &mut BlockStmtOrExpr) {
        body.visit_mut_children_with(self);

        match body {
            BlockStmtOrExpr::BlockStmt(b) => self.optimize_fn_stmts(&mut b.stmts),
            BlockStmtOrExpr::Expr(_) => {}
        }

        self.optimize_arrow_body(body);
    }

    fn visit_mut_call_expr(&mut self, e: &mut CallExpr) {
        {
            let ctx = Ctx {
                is_callee: true,
                ..self.ctx
            };
            e.callee.visit_mut_with(&mut *self.with_ctx(ctx));
        }

        e.args.visit_mut_with(self);

        self.drop_arguments_of_symbol_call(e);
    }

    fn visit_mut_cond_expr(&mut self, e: &mut CondExpr) {
        e.visit_mut_children_with(self);

        self.optimize_expr_in_bool_ctx(&mut e.test);

        self.negate_cond_expr(e);
    }

    fn visit_mut_expr(&mut self, e: &mut Expr) {
        {
            let ctx = Ctx {
                in_first_expr: false,
                ..self.ctx
            };
            e.visit_mut_children_with(&mut *self.with_ctx(ctx));
        }

        self.remove_invalid(e);

        self.drop_console(e);

        self.remove_invalid(e);

        if let Expr::Seq(seq) = e {
            if seq.exprs.is_empty() {
                *e = Expr::Invalid(Invalid { span: DUMMY_SP });
                return;
            }
            if seq.exprs.len() == 1 {
                self.changed = true;
                *e = *seq.exprs.take().into_iter().next().unwrap();
            }
        }

        self.unsafe_optimize_fn_as_arrow(e);

        self.eval_opt_chain(e);

        self.eval_number_call(e);

        self.eval_number_method_call(e);

        self.swap_bin_operands(e);

        self.handle_property_access(e);

        self.optimize_bools(e);

        self.drop_logical_operands(e);

        self.lift_minus(e);

        self.convert_tpl_to_str(e);

        self.drop_useless_addition_of_str(e);

        self.compress_useless_deletes(e);

        self.remove_useless_logical_rhs(e);

        self.handle_negated_seq(e);

        self.concat_str(e);

        self.eval_array_method_call(e);

        self.eval_fn_method_call(e);

        self.eval_str_method_call(e);

        self.compress_conds_as_logical(e);

        self.compress_cond_with_logical_as_logical(e);

        self.lift_seqs_of_bin(e);

        self.lift_seqs_of_cond_assign(e);
    }

    fn visit_mut_expr_stmt(&mut self, s: &mut ExprStmt) {
        s.visit_mut_children_with(self);

        self.ignore_return_value(
            &mut s.expr,
            DropOpts {
                drop_zero: true,
                drop_global_refs_if_unused: true,
                drop_str_lit: false,
            },
        );
    }

    fn visit_mut_exprs(&mut self, exprs: &mut Vec<Box<Expr>>) {
        self.visit_par(exprs);
    }

    fn visit_mut_for_in_stmt(&mut self, n: &mut ForInStmt) {
        n.right.visit_mut_with(self);

        n.left.visit_mut_with(self);

        n.body.visit_mut_with(self);

        if let Stmt::Block(body) = &mut *n.body {
            self.negate_if_terminate(&mut body.stmts, false, true);
        }
    }

    fn visit_mut_for_of_stmt(&mut self, n: &mut ForOfStmt) {
        n.right.visit_mut_with(self);

        n.left.visit_mut_with(self);

        n.body.visit_mut_with(self);

        if let Stmt::Block(body) = &mut *n.body {
            self.negate_if_terminate(&mut body.stmts, false, true);
        }
    }

    fn visit_mut_for_stmt(&mut self, s: &mut ForStmt) {
        s.visit_mut_children_with(self);

        self.merge_for_if_break(s);

        if let Some(test) = &mut s.test {
            self.optimize_expr_in_bool_ctx(&mut **test);
        }

        if let Stmt::Block(body) = &mut *s.body {
            self.negate_if_terminate(&mut body.stmts, false, true);
        }
    }

    fn visit_mut_function(&mut self, f: &mut Function) {
        {
            let ctx = Ctx {
                _in_try_block: false,
                ..self.ctx
            };
            f.visit_mut_children_with(&mut *self.with_ctx(ctx));
        }

        if let Some(body) = &mut f.body {
            self.optimize_fn_stmts(&mut body.stmts)
        }
    }

    fn visit_mut_if_stmt(&mut self, s: &mut IfStmt) {
        s.visit_mut_children_with(self);

        self.optimize_expr_in_bool_ctx(&mut s.test);
    }

    fn visit_mut_member_expr(&mut self, e: &mut MemberExpr) {
        e.obj.visit_mut_with(self);
        if let MemberProp::Computed(c) = &mut e.prop {
            c.visit_mut_with(self);

            // TODO: unify these two
            if let Some(ident) = self.optimize_property_of_member_expr(Some(&e.obj), c) {
                e.prop = MemberProp::Ident(ident);
                return;
            };

            if let Some(ident) = self.handle_known_computed_member_expr(c) {
                e.prop = MemberProp::Ident(ident)
            };
        }
    }

    fn visit_mut_module_items(&mut self, items: &mut Vec<ModuleItem>) {
        self.bindings = Some(Arc::new(collect_decls(items)));

        self.visit_par(items);

        self.handle_stmt_likes(items);
    }

    fn visit_mut_new_expr(&mut self, e: &mut NewExpr) {
        {
            let ctx = Ctx {
                is_callee: true,
                ..self.ctx
            };
            e.callee.visit_mut_with(&mut *self.with_ctx(ctx));
        }

        e.args.visit_mut_with(self);
    }

    fn visit_mut_opt_var_decl_or_expr(&mut self, n: &mut Option<VarDeclOrExpr>) {
        n.visit_mut_children_with(self);

        if self.options.side_effects {
            if let Some(VarDeclOrExpr::Expr(e)) = n {
                self.ignore_return_value(
                    e,
                    DropOpts {
                        drop_zero: true,
                        drop_global_refs_if_unused: true,
                        drop_str_lit: true,
                        ..Default::default()
                    },
                );
                if e.is_invalid() {
                    *n = None;
                }
            }
        }
    }

    fn visit_mut_pat_or_expr(&mut self, n: &mut PatOrExpr) {
        n.visit_mut_children_with(self);

        match n {
            PatOrExpr::Expr(e) => {
                //
                if let Expr::Ident(i) = &mut **e {
                    *n = PatOrExpr::Pat(i.clone().into());
                }
            }
            PatOrExpr::Pat(_) => {}
        }
    }

    fn visit_mut_prop(&mut self, p: &mut Prop) {
        p.visit_mut_children_with(self);

        self.optimize_arrow_method_prop(p);

        if cfg!(debug_assertions) {
            p.visit_with(&mut AssertValid);
        }
    }

    fn visit_mut_prop_name(&mut self, p: &mut PropName) {
        p.visit_mut_children_with(self);

        self.optimize_computed_prop_name_as_normal(p);
        self.optimize_prop_name(p);
    }

    fn visit_mut_prop_or_spreads(&mut self, exprs: &mut Vec<PropOrSpread>) {
        self.visit_par(exprs);
    }

    fn visit_mut_return_stmt(&mut self, s: &mut ReturnStmt) {
        s.visit_mut_children_with(self);

        self.drop_undefined_from_return_arg(s);
    }

    fn visit_mut_seq_expr(&mut self, e: &mut SeqExpr) {
        e.visit_mut_children_with(self);

        e.exprs.retain(|e| {
            if e.is_invalid() {
                self.changed = true;
                tracing::debug!("Removing invalid expr in seq");
                return false;
            }

            true
        });

        if e.exprs.is_empty() {
            return;
        }

        self.merge_seq_call(e);

        let len = e.exprs.len();
        for (idx, e) in e.exprs.iter_mut().enumerate() {
            let is_last = idx == len - 1;

            if !is_last {
                self.ignore_return_value(
                    &mut **e,
                    DropOpts {
                        drop_zero: false,
                        drop_global_refs_if_unused: false,
                        drop_str_lit: true,
                    },
                );
            }
        }

        e.exprs.retain(|e| !e.is_invalid());

        if cfg!(debug_assertions) {
            e.visit_with(&mut AssertValid);
        }
    }

    fn visit_mut_stmt(&mut self, s: &mut Stmt) {
        let _tracing = if cfg!(feature = "debug") && self.debug_infinite_loop {
            let text = dump(&*s, false);

            if text.lines().count() < 10 {
                Some(span!(Level::ERROR, "visit_mut_stmt", "start" = &*text).entered())
            } else {
                None
            }
        } else {
            None
        };

        {
            let ctx = Ctx {
                is_update_arg: false,
                is_callee: false,
                in_delete: false,
                in_first_expr: true,
                ..self.ctx
            };
            s.visit_mut_children_with(&mut *self.with_ctx(ctx));
        }

        if cfg!(feature = "debug") && self.debug_infinite_loop {
            let text = dump(&*s, false);

            if text.lines().count() < 10 {
                tracing::debug!("after: visit_mut_children_with: {}", text);
            }
        }

        if self.options.drop_debugger {
            if let Stmt::Debugger(..) = s {
                self.changed = true;
                *s = Stmt::Empty(EmptyStmt { span: DUMMY_SP });
                tracing::debug!("drop_debugger: Dropped a debugger statement");
                return;
            }
        }

        self.loop_to_for_stmt(s);

        if let Stmt::Expr(es) = s {
            if es.expr.is_invalid() {
                *s = Stmt::Empty(EmptyStmt { span: DUMMY_SP });
                return;
            }
        }

        if cfg!(feature = "debug") && self.debug_infinite_loop {
            let text = dump(&*s, false);

            if text.lines().count() < 10 {
                tracing::debug!("after: visit_mut_stmt: {}", text);
            }
        }

        if cfg!(debug_assertions) {
            s.visit_with(&mut AssertValid);
        }
    }

    fn visit_mut_stmts(&mut self, items: &mut Vec<Stmt>) {
        if !items.is_empty() {
            if let Stmt::Expr(ExprStmt { expr, .. }) = &items[0] {
                if let Expr::Lit(Lit::Str(v)) = &**expr {
                    if v.value == *"use asm" {
                        return;
                    }
                }
            }
        }

        self.visit_par(items);

        self.handle_stmt_likes(items);

        items.retain(|s| !matches!(s, Stmt::Empty(..)));

        if cfg!(debug_assertions) {
            items.visit_with(&mut AssertValid);
        }
    }

    fn visit_mut_super_prop_expr(&mut self, e: &mut SuperPropExpr) {
        if let SuperProp::Computed(c) = &mut e.prop {
            c.visit_mut_with(self);

            if let Some(ident) = self.optimize_property_of_member_expr(None, c) {
                e.prop = SuperProp::Ident(ident);
                return;
            };

            if let Some(ident) = self.handle_known_computed_member_expr(c) {
                e.prop = SuperProp::Ident(ident)
            };
        }
    }

    /// We don't optimize [Tpl] contained in [TaggedTpl].
    fn visit_mut_tagged_tpl(&mut self, n: &mut TaggedTpl) {
        n.tag.visit_mut_with(self);
    }

    fn visit_mut_tpl(&mut self, n: &mut Tpl) {
        n.visit_mut_children_with(self);
        debug_assert_eq!(n.exprs.len() + 1, n.quasis.len());

        self.compress_tpl(n);

        debug_assert_eq!(
            n.exprs.len() + 1,
            n.quasis.len(),
            "tagged template literal compressor created an invalid template literal"
        );
    }

    fn visit_mut_try_stmt(&mut self, n: &mut TryStmt) {
        let ctx = Ctx {
            _in_try_block: true,
            ..self.ctx
        };
        n.block.visit_mut_with(&mut *self.with_ctx(ctx));

        n.handler.visit_mut_with(self);

        n.finalizer.visit_mut_with(self);
    }

    fn visit_mut_unary_expr(&mut self, e: &mut UnaryExpr) {
        {
            let ctx = Ctx {
                in_delete: e.op == op!("delete"),
                ..self.ctx
            };
            e.visit_mut_children_with(&mut *self.with_ctx(ctx));
        }

        match e.op {
            op!("!") => {
                self.optimize_expr_in_bool_ctx(&mut e.arg);
            }

            op!(unary, "+") | op!(unary, "-") => {
                self.optimize_expr_in_num_ctx(&mut e.arg);
            }
            _ => {}
        }
    }

    fn visit_mut_update_expr(&mut self, e: &mut UpdateExpr) {
        let ctx = Ctx {
            is_update_arg: true,
            ..self.ctx
        };

        e.visit_mut_children_with(&mut *self.with_ctx(ctx));
    }

    fn visit_mut_while_stmt(&mut self, s: &mut WhileStmt) {
        s.visit_mut_children_with(self);

        self.optimize_expr_in_bool_ctx(&mut s.test);
    }
}
