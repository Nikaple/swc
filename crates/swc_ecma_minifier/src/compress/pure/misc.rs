use std::num::FpCategory;

use swc_common::{util::take::Take, DUMMY_SP};
use swc_ecma_ast::*;
use swc_ecma_utils::ident::IdentLike;

use super::Pure;
use crate::compress::util::{is_global_var, is_pure_undefined};

impl Pure<'_> {
    pub(super) fn remove_invalid(&mut self, e: &mut Expr) {
        if let Expr::Bin(BinExpr { left, right, .. }) = e {
            self.remove_invalid(left);
            self.remove_invalid(right);

            if left.is_invalid() {
                *e = *right.take();
                self.remove_invalid(e);
            } else if right.is_invalid() {
                *e = *left.take();
                self.remove_invalid(e);
            }
        }
    }

    pub(super) fn drop_undefined_from_return_arg(&mut self, s: &mut ReturnStmt) {
        if let Some(e) = s.arg.as_deref() {
            if is_pure_undefined(e) {
                self.changed = true;
                tracing::debug!("Dropped `undefined` from `return undefined`");
                s.arg.take();
            }
        }
    }

    pub(super) fn remove_useless_return(&mut self, stmts: &mut Vec<Stmt>) {
        if let Some(Stmt::Return(ReturnStmt { arg: None, .. })) = stmts.last() {
            self.changed = true;
            tracing::debug!("misc: Removing useless return");
            stmts.pop();
        }
    }

    /// Removes last return statement. This should be callled only if the return
    /// value of function is ignored.
    ///
    /// Returns true if something is modified.
    fn drop_return_value(&mut self, stmts: &mut Vec<Stmt>) -> bool {
        for s in stmts.iter_mut() {
            if let Stmt::Return(ReturnStmt {
                arg: arg @ Some(..),
                ..
            }) = s
            {
                self.ignore_return_value(
                    arg.as_deref_mut().unwrap(),
                    DropOpts {
                        drop_global_refs_if_unused: true,
                        drop_zero: true,
                        drop_str_lit: true,
                        ..Default::default()
                    },
                );

                if let Some(Expr::Invalid(..)) = arg.as_deref() {
                    self.changed = true;
                    *arg = None;
                }
            }
        }

        if let Some(last) = stmts.last_mut() {
            self.drop_return_value_of_stmt(last)
        } else {
            false
        }
    }

    /// Returns true if something is modified.
    fn drop_return_value_of_stmt(&mut self, s: &mut Stmt) -> bool {
        match s {
            Stmt::Block(s) => self.drop_return_value(&mut s.stmts),
            Stmt::Return(ret) => {
                self.changed = true;
                if cfg!(feature = "debug") {
                    tracing::trace!("Dropping `return` token");
                }

                let span = ret.span;
                match ret.arg.take() {
                    Some(arg) => {
                        *s = Stmt::Expr(ExprStmt { span, expr: arg });
                    }
                    None => {
                        *s = Stmt::Empty(EmptyStmt { span });
                    }
                }

                true
            }

            Stmt::Labeled(s) => self.drop_return_value_of_stmt(&mut s.body),
            Stmt::If(s) => {
                let c = self.drop_return_value_of_stmt(&mut s.cons);
                let a = s
                    .alt
                    .as_deref_mut()
                    .map(|s| self.drop_return_value_of_stmt(s))
                    .unwrap_or_default();

                c || a
            }

            Stmt::Try(s) => {
                let a = if s.finalizer.is_none() {
                    self.drop_return_value(&mut s.block.stmts)
                } else {
                    false
                };

                let b = s
                    .finalizer
                    .as_mut()
                    .map(|s| self.drop_return_value(&mut s.stmts))
                    .unwrap_or_default();

                a || b
            }

            _ => false,
        }
    }

    fn make_ignored_expr(&mut self, exprs: impl Iterator<Item = Box<Expr>>) -> Option<Expr> {
        let mut exprs = exprs
            .filter_map(|mut e| {
                self.ignore_return_value(
                    &mut *e,
                    DropOpts {
                        drop_global_refs_if_unused: true,
                        drop_str_lit: true,
                        drop_zero: true,
                    },
                );

                if let Expr::Invalid(..) = &*e {
                    None
                } else {
                    Some(e)
                }
            })
            .collect::<Vec<_>>();

        if exprs.is_empty() {
            return None;
        }
        if exprs.len() == 1 {
            return Some(*exprs.remove(0));
        }

        Some(Expr::Seq(SeqExpr {
            span: DUMMY_SP,
            exprs,
        }))
    }

    #[inline(never)]
    pub(super) fn ignore_return_value(&mut self, e: &mut Expr, opts: DropOpts) {
        match e {
            Expr::Seq(seq) => {
                if seq.exprs.is_empty() {
                    e.take();
                    return;
                }
            }

            Expr::Call(CallExpr { span, args, .. }) if span.has_mark(self.marks.pure) => {
                tracing::debug!("ignore_return_value: Dropping a pure call");
                self.changed = true;

                let new = self.make_ignored_expr(args.take().into_iter().map(|arg| arg.expr));

                *e = new.unwrap_or(Expr::Invalid(Invalid { span: DUMMY_SP }));
                return;
            }

            Expr::TaggedTpl(TaggedTpl {
                span,
                tpl: Tpl { exprs, .. },
                ..
            }) if span.has_mark(self.marks.pure) => {
                tracing::debug!("ignore_return_value: Dropping a pure call");
                self.changed = true;

                let new = self.make_ignored_expr(exprs.take().into_iter());

                *e = new.unwrap_or(Expr::Invalid(Invalid { span: DUMMY_SP }));
                return;
            }

            Expr::New(NewExpr { span, args, .. }) if span.has_mark(self.marks.pure) => {
                tracing::debug!("ignore_return_value: Dropping a pure call");
                self.changed = true;

                let new =
                    self.make_ignored_expr(args.take().into_iter().flatten().map(|arg| arg.expr));

                *e = new.unwrap_or(Expr::Invalid(Invalid { span: DUMMY_SP }));
                return;
            }

            _ => {}
        }

        if self.options.unused {
            if let Expr::Lit(Lit::Num(n)) = e {
                // Skip 0
                if n.value != 0.0 && n.value.classify() == FpCategory::Normal {
                    self.changed = true;
                    *e = Expr::Invalid(Invalid { span: DUMMY_SP });
                    return;
                }
            }
        }

        if let Expr::Ident(i) = e {
            // If it's not a top level, it's a reference to a declared variable.
            if i.span.ctxt.outer() == self.marks.top_level_mark {
                if self.options.side_effects
                    || (self.options.unused && opts.drop_global_refs_if_unused)
                {
                    if is_global_var(&i.sym) {
                        tracing::debug!("Dropping a reference to a global variable");
                        *e = Expr::Invalid(Invalid { span: DUMMY_SP });
                        return;
                    }
                }
            } else {
                tracing::debug!("Dropping an identifier as it's declared");
                *e = Expr::Invalid(Invalid { span: DUMMY_SP });
                return;
            }
        }

        if self.options.side_effects {
            match e {
                Expr::Unary(UnaryExpr {
                    op: op!("void") | op!("typeof") | op!(unary, "+") | op!(unary, "-"),
                    arg,
                    ..
                }) => {
                    self.ignore_return_value(
                        &mut **arg,
                        DropOpts {
                            drop_str_lit: true,
                            drop_global_refs_if_unused: true,
                            drop_zero: true,
                            ..opts
                        },
                    );

                    if arg.is_invalid() {
                        tracing::debug!("Dropping an unary expression");
                        *e = Expr::Invalid(Invalid { span: DUMMY_SP });
                        return;
                    }
                }

                Expr::Bin(
                    be @ BinExpr {
                        op: op!("||") | op!("&&"),
                        ..
                    },
                ) => {
                    self.ignore_return_value(&mut be.right, opts);

                    if be.right.is_invalid() {
                        tracing::debug!("Dropping the RHS of a binary expression ('&&' / '||')");
                        *e = *be.left.take();
                        return;
                    }
                }

                _ => {}
            }
        }

        if self.options.unused || self.options.side_effects {
            match e {
                Expr::Lit(Lit::Num(n)) => {
                    if n.value == 0.0 && opts.drop_zero {
                        self.changed = true;
                        *e = Expr::Invalid(Invalid { span: DUMMY_SP });
                        return;
                    }
                }

                Expr::Ident(i) => {
                    if let Some(bindings) = self.bindings.as_deref() {
                        if bindings.contains(&i.to_id()) {
                            tracing::debug!("Dropping an identifier as it's declared");

                            self.changed = true;
                            *e = Expr::Invalid(Invalid { span: DUMMY_SP });
                            return;
                        }
                    }
                }

                Expr::Lit(Lit::Null(..) | Lit::BigInt(..) | Lit::Bool(..) | Lit::Regex(..)) => {
                    tracing::debug!("Dropping literals");

                    self.changed = true;
                    *e = Expr::Invalid(Invalid { span: DUMMY_SP });
                    return;
                }

                Expr::Bin(
                    bin @ BinExpr {
                        op:
                            op!(bin, "+")
                            | op!(bin, "-")
                            | op!("*")
                            | op!("/")
                            | op!("%")
                            | op!("**")
                            | op!("^")
                            | op!("&")
                            | op!("|")
                            | op!(">>")
                            | op!("<<")
                            | op!(">>>")
                            | op!("===")
                            | op!("!==")
                            | op!("==")
                            | op!("!=")
                            | op!("<")
                            | op!("<=")
                            | op!(">")
                            | op!(">="),
                        ..
                    },
                ) => {
                    self.ignore_return_value(
                        &mut bin.left,
                        DropOpts {
                            drop_zero: true,
                            drop_global_refs_if_unused: true,
                            drop_str_lit: true,
                            ..opts
                        },
                    );
                    self.ignore_return_value(
                        &mut bin.right,
                        DropOpts {
                            drop_zero: true,
                            drop_global_refs_if_unused: true,
                            drop_str_lit: true,
                            ..opts
                        },
                    );
                    let span = bin.span;

                    if bin.left.is_invalid() && bin.right.is_invalid() {
                        *e = Expr::Invalid(Invalid { span: DUMMY_SP });
                        return;
                    } else if bin.right.is_invalid() {
                        *e = *bin.left.take();
                        return;
                    } else if bin.left.is_invalid() {
                        *e = *bin.right.take();
                        return;
                    }

                    if bin.left.is_await_expr() {
                        self.changed = true;
                        tracing::debug!("ignore_return_value: Left is await");
                        *e = Expr::Seq(SeqExpr {
                            span,
                            exprs: vec![bin.left.take(), bin.right.take()],
                        });
                        return;
                    }
                }

                Expr::Assign(assign @ AssignExpr { op: op!("="), .. }) => {
                    // Convert `a = a` to `a`.
                    if let Some(l) = assign.left.as_ident() {
                        if let Expr::Ident(r) = &*assign.right {
                            if l.to_id() == r.to_id() {
                                self.changed = true;
                                *e = *assign.right.take();
                            }
                        }
                    }
                }

                _ => {}
            }
        }

        match e {
            Expr::Lit(Lit::Str(s)) => {
                if opts.drop_str_lit
                    || (s.value.starts_with("@swc/helpers")
                        || s.value.starts_with("@babel/helpers"))
                {
                    self.changed = true;
                    *e = Expr::Invalid(Invalid { span: DUMMY_SP });
                    return;
                }
            }

            Expr::Seq(e) => {
                self.drop_useless_ident_ref_in_seq(e);

                if let Some(last) = e.exprs.last_mut() {
                    // Non-last elements are already processed.
                    self.ignore_return_value(&mut **last, opts);
                }

                let len = e.exprs.len();
                e.exprs.retain(|e| !e.is_invalid());
                if e.exprs.len() != len {
                    self.changed = true;
                }
                return;
            }

            Expr::Call(CallExpr {
                callee: Callee::Expr(callee),
                ..
            }) if callee.is_fn_expr() => match &mut **callee {
                Expr::Fn(callee) => {
                    if callee.ident.is_none() {
                        if let Some(body) = &mut callee.function.body {
                            if self.options.side_effects {
                                self.drop_return_value(&mut body.stmts);
                            }
                        }
                    }
                }

                _ => {
                    unreachable!()
                }
            },

            _ => {}
        }

        // Remove pure member expressions.
        if let Expr::Member(MemberExpr { obj, prop, .. }) = e {
            if let Expr::Ident(obj) = &**obj {
                if obj.span.ctxt.outer() == self.marks.top_level_mark {
                    if let Some(bindings) = self.bindings.as_deref() {
                        if !bindings.contains(&obj.to_id()) {
                            if is_pure_member_access(obj, prop) {
                                self.changed = true;
                                *e = Expr::Invalid(Invalid { span: DUMMY_SP });
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct DropOpts {
    /// If true and `unused` option is enabled, references to global variables
    /// will be dropped, even if `side_effects` is false.
    pub drop_global_refs_if_unused: bool,
    pub drop_zero: bool,
    pub drop_str_lit: bool,
}

/// `obj` should have top level syntax context.
fn is_pure_member_access(obj: &Ident, prop: &MemberProp) -> bool {
    macro_rules! check {
        (
            $obj:ident.
            $prop:ident
        ) => {{
            if &*obj.sym == stringify!($obj) {
                if let MemberProp::Ident(prop) = prop {
                    if &*prop.sym == stringify!($prop) {
                        return true;
                    }
                }
            }
        }};
    }

    macro_rules! pure {
        (
            $(
                $(
                  $i:ident
                ).*
            ),*
        ) => {
            $(
                check!($($i).*);
            )*
        };
    }

    pure!(
        Array.isArray,
        ArrayBuffer.isView,
        Boolean.toSource,
        Date.parse,
        Date.UTC,
        Date.now,
        Error.captureStackTrace,
        Error.stackTraceLimit,
        Function.bind,
        Function.call,
        Function.length,
        console.log,
        Error.name,
        Math.random,
        Number.isNaN,
        Object.defineProperty,
        String.fromCharCode
    );

    false
}
