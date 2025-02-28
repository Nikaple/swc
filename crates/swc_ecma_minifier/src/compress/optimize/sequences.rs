use std::mem::take;

use retain_mut::RetainMut;
use swc_atoms::js_word;
use swc_common::{util::take::Take, Spanned, DUMMY_SP};
use swc_ecma_ast::*;
use swc_ecma_utils::{
    contains_arguments, contains_this_expr, ident::IdentLike, undefined, ExprExt, Id, StmtLike,
    UsageFinder,
};
use swc_ecma_visit::{noop_visit_type, Visit, VisitWith};
use tracing::{span, Level};

use super::{is_pure_undefined, Optimizer};
use crate::{
    compress::{
        optimize::util::replace_id_with_expr,
        util::{is_directive, is_ident_used_by, replace_expr},
    },
    debug::dump,
    mode::Mode,
    option::CompressOptions,
    util::{idents_used_by, idents_used_by_ignoring_nested, ExprOptExt, ModuleItemExt},
};

/// Methods related to the option `sequences`. All methods are noop if
/// `sequences` is false.
impl<M> Optimizer<'_, M>
where
    M: Mode,
{
    ///
    /// # Example
    ///
    ///
    /// ## Input
    ///
    /// ```ts
    /// x = 5;
    /// if (y) z();
    /// x = 5;
    /// for (i = 0; i < 5; i++) console.log(i);
    /// x = 5;
    /// for (; i < 5; i++) console.log(i);
    /// x = 5;
    /// switch (y) {
    /// }
    /// x = 5;
    /// with (obj) {
    /// }
    /// ```
    ///
    /// ## Output
    /// ```ts
    /// if (x = 5, y) z();
    /// for(x = 5, i = 0; i < 5; i++)console.log(i);
    /// for(x = 5; i < 5; i++)console.log(i);
    /// switch(x = 5, y){
    /// }
    /// with (x = 5, obj);
    /// ```
    pub(super) fn make_sequences<T>(&mut self, stmts: &mut Vec<T>)
    where
        T: StmtLike,
    {
        if !self.options.sequences() {
            return;
        }
        if self.ctx.in_asm {
            return;
        }

        {
            let can_work =
                stmts
                    .windows(2)
                    .any(|stmts| match (stmts[0].as_stmt(), stmts[1].as_stmt()) {
                        (Some(l @ Stmt::Expr(..)), Some(r)) => {
                            if is_directive(l) || is_directive(r) {
                                return false;
                            }

                            // If an expression contains `in` and following statement is for loop,
                            // we should not merge it.

                            // TODO: Check for `in`

                            match r {
                                Stmt::Expr(..)
                                | Stmt::If(..)
                                | Stmt::Switch(..)
                                | Stmt::With(..)
                                | Stmt::Return(ReturnStmt { arg: Some(..), .. })
                                | Stmt::Throw(ThrowStmt { .. })
                                | Stmt::For(ForStmt { init: None, .. })
                                | Stmt::For(ForStmt {
                                    init: Some(VarDeclOrExpr::Expr(..)),
                                    ..
                                })
                                | Stmt::ForIn(..)
                                | Stmt::ForOf(..) => true,

                                Stmt::Decl(Decl::Var(
                                    v @ VarDecl {
                                        kind: VarDeclKind::Var,
                                        ..
                                    },
                                )) => v.decls.iter().all(|vd| vd.init.is_none()),

                                Stmt::Decl(Decl::Fn(..)) => true,

                                _ => false,
                            }
                        }
                        _ => false,
                    });

            if !can_work {
                return;
            }

            if stmts.len() == 2 {
                match stmts[1].as_stmt() {
                    Some(Stmt::Decl(Decl::Var(
                        v @ VarDecl {
                            kind: VarDeclKind::Var,
                            ..
                        },
                    ))) => {
                        if v.decls.iter().all(|vd| vd.init.is_none()) {
                            return;
                        }
                    }

                    Some(Stmt::Decl(Decl::Fn(..))) => return,

                    _ => {}
                }
            }
        }

        tracing::debug!("sequences: Compressing statements as a sequences");

        self.changed = true;
        let mut exprs = vec![];
        // This is bigger than required.
        let mut new_stmts = Vec::with_capacity(stmts.len());

        for stmt in stmts.take() {
            match stmt.try_into_stmt() {
                Ok(stmt) => {
                    if is_directive(&stmt) {
                        new_stmts.push(T::from_stmt(stmt));
                        continue;
                    }
                    // If
                    match stmt {
                        Stmt::Expr(stmt) => {
                            exprs.push(stmt.expr);
                        }

                        Stmt::If(mut stmt) => {
                            stmt.test.prepend_exprs(take(&mut exprs));
                            new_stmts.push(T::from_stmt(Stmt::If(stmt)));
                        }

                        Stmt::Switch(mut stmt) => {
                            stmt.discriminant.prepend_exprs(take(&mut exprs));

                            new_stmts.push(T::from_stmt(Stmt::Switch(stmt)));
                        }

                        Stmt::With(mut stmt) => {
                            stmt.obj.prepend_exprs(take(&mut exprs));

                            new_stmts.push(T::from_stmt(Stmt::With(stmt)));
                        }

                        Stmt::Return(mut stmt @ ReturnStmt { arg: Some(..), .. }) => {
                            match stmt.arg.as_deref_mut() {
                                Some(e) => {
                                    e.prepend_exprs(take(&mut exprs));
                                }
                                _ => {
                                    let mut e = undefined(stmt.span);
                                    e.prepend_exprs(take(&mut exprs));

                                    stmt.arg = Some(e);
                                }
                            }

                            new_stmts.push(T::from_stmt(Stmt::Return(stmt)));
                        }

                        Stmt::Throw(mut stmt) => {
                            stmt.arg.prepend_exprs(take(&mut exprs));

                            new_stmts.push(T::from_stmt(Stmt::Throw(stmt)));
                        }

                        Stmt::For(mut stmt @ ForStmt { init: None, .. })
                        | Stmt::For(
                            mut stmt @ ForStmt {
                                init: Some(VarDeclOrExpr::Expr(..)),
                                ..
                            },
                        ) => {
                            match &mut stmt.init {
                                Some(VarDeclOrExpr::Expr(e)) => {
                                    if exprs.iter().all(|expr| {
                                        matches!(
                                            &**expr,
                                            Expr::Assign(AssignExpr { op: op!("="), .. })
                                        )
                                    }) {
                                        let ids_used_by_exprs =
                                            idents_used_by_ignoring_nested(&exprs);

                                        let ids_used_by_first_expr =
                                            idents_used_by_ignoring_nested(&*e.first_expr_mut());

                                        let has_conflict = ids_used_by_exprs
                                            .iter()
                                            .any(|id| ids_used_by_first_expr.contains(id));

                                        // I(kdy1) don't know why we need this, but terser appends
                                        // instead of prependig if initializer is (exactly)
                                        //
                                        // "identifier" = "literal".
                                        //
                                        // Note that only the form above makes terser to append.
                                        //
                                        // When I tested in by changing input multiple times, terser
                                        // seems to be aware of side effects.
                                        //
                                        // Maybe there exists an optimization related to it in v8.
                                        if let Expr::Assign(AssignExpr {
                                            op: op!("="),
                                            left,
                                            right,
                                            ..
                                        }) = e.first_expr_mut()
                                        {
                                            if !has_conflict
                                                && left.as_ident().is_some()
                                                && match &**right {
                                                    Expr::Lit(Lit::Regex(..)) => false,
                                                    Expr::Lit(..) => true,
                                                    _ => false,
                                                }
                                            {
                                                let seq = e.force_seq();
                                                let extra =
                                                    seq.exprs.drain(1..).collect::<Vec<_>>();
                                                seq.exprs.extend(take(&mut exprs));
                                                seq.exprs.extend(extra);

                                                new_stmts.push(T::from_stmt(Stmt::For(stmt)));

                                                continue;
                                            }
                                        }
                                    }
                                    e.prepend_exprs(take(&mut exprs));
                                }
                                None => {
                                    stmt.init =
                                        Some(VarDeclOrExpr::Expr(Box::new(Expr::Seq(SeqExpr {
                                            span: DUMMY_SP,
                                            exprs: take(&mut exprs),
                                        }))))
                                }
                                _ => {
                                    unreachable!()
                                }
                            }
                            new_stmts.push(T::from_stmt(Stmt::For(stmt)));
                        }

                        Stmt::ForIn(mut stmt) => {
                            stmt.right.prepend_exprs(take(&mut exprs));

                            new_stmts.push(T::from_stmt(Stmt::ForIn(stmt)));
                        }

                        Stmt::ForOf(mut stmt) => {
                            stmt.right.prepend_exprs(take(&mut exprs));

                            new_stmts.push(T::from_stmt(Stmt::ForOf(stmt)));
                        }

                        Stmt::Decl(Decl::Var(
                            var @ VarDecl {
                                kind: VarDeclKind::Var,
                                ..
                            },
                        )) if var.decls.iter().all(|v| v.init.is_none()) => {
                            new_stmts.push(T::from_stmt(Stmt::Decl(Decl::Var(var))));
                        }

                        Stmt::Decl(Decl::Fn(..)) => {
                            new_stmts.push(T::from_stmt(stmt));
                        }

                        _ => {
                            if !exprs.is_empty() {
                                new_stmts.push(T::from_stmt(Stmt::Expr(ExprStmt {
                                    span: DUMMY_SP,
                                    expr: Box::new(Expr::Seq(SeqExpr {
                                        span: DUMMY_SP,
                                        exprs: take(&mut exprs),
                                    })),
                                })))
                            }

                            new_stmts.push(T::from_stmt(stmt));
                        }
                    }
                }
                Err(item) => {
                    if !exprs.is_empty() {
                        new_stmts.push(T::from_stmt(Stmt::Expr(ExprStmt {
                            span: DUMMY_SP,
                            expr: Box::new(Expr::Seq(SeqExpr {
                                span: DUMMY_SP,
                                exprs: take(&mut exprs),
                            })),
                        })))
                    }

                    new_stmts.push(item);
                }
            }
        }

        if !exprs.is_empty() {
            new_stmts.push(T::from_stmt(Stmt::Expr(ExprStmt {
                span: DUMMY_SP,
                expr: Box::new(Expr::Seq(SeqExpr {
                    span: DUMMY_SP,
                    exprs: take(&mut exprs),
                })),
            })))
        }

        *stmts = new_stmts;
    }

    /// Break assignments in sequences.
    ///
    /// This may result in less parenthesis.
    pub(super) fn break_assignments_in_seqs<T>(&mut self, stmts: &mut Vec<T>)
    where
        T: StmtLike,
    {
        // TODO
        if true {
            return;
        }
        let need_work = stmts.iter().any(|stmt| match stmt.as_stmt() {
            Some(Stmt::Expr(e)) => match &*e.expr {
                Expr::Seq(seq) => {
                    seq.exprs.len() > 1
                        && seq.exprs.iter().all(|expr| {
                            matches!(&**expr, Expr::Assign(AssignExpr { op: op!("="), .. }))
                        })
                }
                _ => false,
            },

            _ => false,
        });

        if !need_work {
            return;
        }

        let mut new_stmts = vec![];

        for stmt in stmts.take() {
            match stmt.try_into_stmt() {
                Ok(stmt) => match stmt {
                    Stmt::Expr(es)
                        if match &*es.expr {
                            Expr::Seq(seq) => {
                                seq.exprs.len() > 1
                                    && seq.exprs.iter().all(|expr| {
                                        matches!(
                                            &**expr,
                                            Expr::Assign(AssignExpr { op: op!("="), .. })
                                        )
                                    })
                            }
                            _ => false,
                        } =>
                    {
                        let span = es.span;
                        let seq = es.expr.seq().unwrap();
                        new_stmts.extend(
                            seq.exprs
                                .into_iter()
                                .map(|expr| ExprStmt { span, expr })
                                .map(Stmt::Expr)
                                .map(T::from_stmt),
                        );
                    }

                    _ => {
                        new_stmts.push(T::from_stmt(stmt));
                    }
                },
                Err(stmt) => {
                    new_stmts.push(stmt);
                }
            }
        }
        self.changed = true;
        tracing::debug!(
            "sequences: Splitted a sequence expression to multiple expression statements"
        );
        *stmts = new_stmts;
    }

    /// Lift sequence expressions in an assign expression.
    ///
    /// - `(a = (f, 4)) => (f, a = 4)`
    pub(super) fn lift_seqs_of_assign(&mut self, e: &mut SeqExpr) {
        if !self.options.sequences() {
            return;
        }

        {
            let can_work = e.exprs.iter().any(|e| {
                if let Expr::Assign(assign @ AssignExpr { op: op!("="), .. }) = &**e {
                    if let Expr::Seq(right) = &*assign.right {
                        if right.exprs.len() >= 2 {
                            return true;
                        }
                    }
                }

                false
            });

            if !can_work {
                return;
            }
            tracing::debug!("sequences: Lifting");
            self.changed = true;
        }

        let mut new_exprs = Vec::with_capacity(e.exprs.len() * 12 / 10);

        for expr in e.exprs.take() {
            if let Expr::Assign(assign @ AssignExpr { op: op!("="), .. }) = *expr {
                match *assign.right {
                    Expr::Seq(mut right) => {
                        new_exprs.extend(right.exprs.drain(..right.exprs.len() - 1));
                        new_exprs.push(Box::new(Expr::Assign(AssignExpr {
                            right: right.exprs.pop().unwrap(),
                            ..assign
                        })));
                        continue;
                    }
                    _ => {
                        new_exprs.push(Box::new(Expr::Assign(assign)));
                        continue;
                    }
                }
            }

            new_exprs.push(expr);
        }

        e.exprs = new_exprs;
    }

    /// Hoist variables in subscope.
    ///
    /// I don't know why it depends on `sequences`.
    pub(super) fn extract_vars_in_subscopes(&mut self, s: &mut Stmt) {
        if !self.options.sequences() {
            return;
        }

        match s {
            Stmt::If(stmt) if self.options.conditionals => {
                self.extract_vars(&mut stmt.cons);
                if let Some(alt) = &mut stmt.alt {
                    self.extract_vars(alt);
                }
            }

            _ => {}
        }
    }

    /// Move `var` in subscope to current scope.
    ///
    /// This method actually `hoist`s [VarDecl]s declared with `var`.
    fn extract_vars(&mut self, s: &mut Stmt) {
        let mut found_other = false;
        if let Stmt::Block(bs) = s {
            // Extract variables without
            for stmt in &mut bs.stmts {
                match stmt {
                    Stmt::Decl(Decl::Var(
                        v @ VarDecl {
                            kind: VarDeclKind::Var,
                            ..
                        },
                    )) => {
                        for decl in &mut v.decls {
                            if decl.init.is_some() {
                                continue;
                            }
                            self.changed = true;
                            tracing::debug!("sequences: Hoisting `var` without init");
                            let s = Stmt::Decl(Decl::Var(VarDecl {
                                span: v.span,
                                kind: VarDeclKind::Var,
                                declare: false,
                                decls: vec![decl.take()],
                            }));
                            if found_other {
                                self.append_stmts.push(s);
                            } else {
                                self.prepend_stmts.push(s);
                            }
                        }

                        v.decls.retain(|v| !v.name.is_invalid());
                    }
                    _ => {
                        found_other = true;
                    }
                }
            }

            bs.stmts.retain(|s| match s {
                Stmt::Empty(..) => false,
                Stmt::Decl(Decl::Var(v)) => !v.decls.is_empty(),
                _ => true,
            });
        }
    }

    ///
    /// - `(path += 'foo', path)` => `(path += 'foo')`
    pub(super) fn shift_assignment(&mut self, e: &mut SeqExpr) {
        if e.exprs.len() < 2 {
            return;
        }

        if let Some(last) = e.exprs.last() {
            let last_id = match &**last {
                Expr::Ident(i) => i,
                _ => return,
            };

            if let Expr::Assign(assign @ AssignExpr { op: op!("="), .. }) =
                &*e.exprs[e.exprs.len() - 2]
            {
                if let Some(lhs) = assign.left.as_ident() {
                    if lhs.sym == last_id.sym && lhs.span.ctxt == last_id.span.ctxt {
                        e.exprs.pop();
                        self.changed = true;
                        tracing::debug!("sequences: Shifting assignment");
                    }
                };
            }
        }
    }

    pub(super) fn shift_void(&mut self, e: &mut SeqExpr) {
        if e.exprs.len() < 2 {
            return;
        }

        if let Expr::Unary(UnaryExpr {
            op: op!("void"), ..
        }) = &*e.exprs[e.exprs.len() - 2]
        {
            return;
        }

        if let Some(last) = e.exprs.last() {
            if is_pure_undefined(last) {
                self.changed = true;
                tracing::debug!("sequences: Shifting void");

                e.exprs.pop();
                let last = e.exprs.last_mut().unwrap();

                *last = Box::new(Expr::Unary(UnaryExpr {
                    span: DUMMY_SP,
                    op: op!("void"),
                    arg: last.take(),
                }))
            }
        }
    }

    fn seq_exprs_of<'a>(
        &mut self,
        s: &'a mut Stmt,
        options: &CompressOptions,
    ) -> Option<Vec<Mergable<'a>>> {
        Some(match s {
            Stmt::Expr(e) => vec![Mergable::Expr(&mut *e.expr)],
            Stmt::Decl(Decl::Var(
                v @ VarDecl {
                    kind: VarDeclKind::Var | VarDeclKind::Let,
                    ..
                },
            )) => {
                if options.reduce_vars || options.collapse_vars {
                    v.decls.iter_mut().map(Mergable::Var).collect()
                } else {
                    return None;
                }
            }
            Stmt::Return(ReturnStmt { arg: Some(arg), .. }) => {
                vec![Mergable::Expr(&mut **arg)]
            }
            Stmt::If(s) if options.sequences() => {
                vec![Mergable::Expr(&mut *s.test)]
            }
            Stmt::Throw(s) if options.sequences() => {
                vec![Mergable::Expr(&mut *s.arg)]
            }

            _ => return None,
        })
    }

    #[cfg_attr(feature = "debug", tracing::instrument(skip_all))]
    pub(super) fn merge_sequences_in_stmts<T>(&mut self, stmts: &mut Vec<T>)
    where
        T: ModuleItemExt,
    {
        if !self.options.sequences() && !self.options.collapse_vars {
            if cfg!(feature = "debug") {
                tracing::trace!("sequences: [x] Disabled");
            }
            return;
        }

        if self.ctx.in_top_level() && !self.options.top_level() {
            if cfg!(feature = "debug") {
                tracing::trace!("sequences: [x] Top level");
            }
            return;
        }

        let mut exprs = vec![];
        let mut buf = vec![];

        for stmt in stmts.iter_mut() {
            let is_end = matches!(
                stmt.as_stmt(),
                Some(Stmt::If(..) | Stmt::Throw(..) | Stmt::Return(..))
            );

            let items = if let Some(stmt) = stmt.as_stmt_mut() {
                self.seq_exprs_of(stmt, self.options)
            } else {
                None
            };
            if let Some(items) = items {
                buf.extend(items)
            } else {
                exprs.push(take(&mut buf));
                continue;
            }
            if is_end {
                exprs.push(take(&mut buf));
            }
        }

        exprs.push(buf);

        let _tracing = if cfg!(feature = "debug") {
            let buf_len = exprs.iter().map(|v| v.len()).collect::<Vec<_>>();
            Some(
                tracing::span!(
                    Level::TRACE,
                    "merge_sequences_in_stmts",
                    items_len = tracing::field::debug(&buf_len)
                )
                .entered(),
            )
        } else {
            None
        };

        for mut exprs in exprs {
            let _ = self.merge_sequences_in_exprs(&mut exprs);
        }

        stmts.retain_mut(|stmt| {
            if let Some(Stmt::Expr(es)) = stmt.as_stmt_mut() {
                if let Expr::Seq(e) = &mut *es.expr {
                    e.exprs.retain(|e| !e.is_invalid());
                }
            }

            match stmt.as_stmt_mut() {
                Some(Stmt::Decl(Decl::Var(v))) => {
                    v.decls
                        .retain(|decl| !matches!(decl.init.as_deref(), Some(Expr::Invalid(..))));

                    !v.decls.is_empty()
                }
                Some(Stmt::Expr(s)) if s.expr.is_invalid() => false,

                _ => true,
            }
        });
    }

    pub(super) fn normalize_sequences(&self, seq: &mut SeqExpr) {
        for e in &mut seq.exprs {
            if let Expr::Seq(e) = &mut **e {
                self.normalize_sequences(&mut *e);
            }
        }

        if seq.exprs.iter().any(|v| v.is_seq()) {
            let mut new = vec![];

            for e in seq.exprs.take() {
                match *e {
                    Expr::Seq(s) => {
                        new.extend(s.exprs);
                    }
                    _ => new.push(e),
                }
            }

            seq.exprs = new;
        }
    }

    pub(super) fn merge_sequences_in_seq_expr(&mut self, e: &mut SeqExpr) {
        self.normalize_sequences(e);

        let _tracing = if cfg!(feature = "debug") {
            let e_str = dump(&*e, false);

            Some(
                span!(
                    Level::ERROR,
                    "merge_sequences_in_seq_expr",
                    seq_expr = &*e_str
                )
                .entered(),
            )
        } else {
            None
        };

        if !self.options.sequences() && !e.span.has_mark(self.marks.synthesized_seq) {
            return;
        }

        let mut exprs = e
            .exprs
            .iter_mut()
            .map(|e| &mut **e)
            .map(Mergable::Expr)
            .collect();

        let _ = self.merge_sequences_in_exprs(&mut exprs);

        e.exprs.retain(|e| !e.is_invalid());
    }

    /// Calls `merge_sequential_expr`.
    ///
    ///
    /// TODO(kdy1): Check for side effects and call merge_sequential_expr more
    /// if expressions between a and b are side-effect-free.
    fn merge_sequences_in_exprs(&mut self, exprs: &mut Vec<Mergable>) -> Result<(), ()> {
        let _tracing = if cfg!(feature = "debug") {
            Some(
                tracing::span!(Level::TRACE, "merge_sequences_in_exprs", len = exprs.len())
                    .entered(),
            )
        } else {
            None
        };

        for idx in 0..exprs.len() {
            for j in idx..exprs.len() {
                let (a1, a2) = exprs.split_at_mut(idx);

                if a1.is_empty() || a2.is_empty() {
                    break;
                }

                let a = a1.last_mut().unwrap();

                if self.merge_sequential_expr(
                    a,
                    match &mut a2[j - idx] {
                        Mergable::Var(b) => match b.init.as_deref_mut() {
                            Some(v) => v,
                            None => continue,
                        },
                        Mergable::Expr(e) => e,
                    },
                )? {
                    break;
                }

                // This logic is required to handle
                //
                // var b;
                // (function () {
                //     function f() {
                //         a++;
                //     }
                //     f();
                //     var c = f();
                //     var a = void 0;
                //     c || (b = a);
                // })();
                // console.log(b);
                //
                //
                // at the code above, c cannot be shifted to `c` in `c || (b = a)`
                //

                match a {
                    Mergable::Var(VarDeclarator {
                        init: Some(init), ..
                    }) => {
                        if !self.is_skippable_for_seq(None, init) {
                            break;
                        }
                    }
                    Mergable::Expr(Expr::Assign(a)) => {
                        if let Some(a) = a.left.as_expr() {
                            if !self.is_skippable_for_seq(None, a) {
                                break;
                            }
                        }

                        if !self.is_skippable_for_seq(None, &a.right) {
                            break;
                        }
                    }
                    _ => {}
                }

                match &a2[j - idx] {
                    Mergable::Var(e2) => {
                        if let Some(e2) = &e2.init {
                            if !self.is_skippable_for_seq(Some(a), e2) {
                                break;
                            }
                        }

                        if let Some(id) = a1.last_mut().unwrap().id() {
                            // TODO(kdy1): Optimize
                            if idents_used_by(&**e2).contains(&id) {
                                break;
                            }
                        }
                    }
                    Mergable::Expr(e2) => {
                        if !self.is_skippable_for_seq(Some(a), e2) {
                            break;
                        }

                        if let Some(id) = a1.last_mut().unwrap().id() {
                            // TODO(kdy1): Optimize
                            if idents_used_by(&**e2).contains(&id) {
                                break;
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn is_skippable_for_seq(&self, a: Option<&Mergable>, e: &Expr) -> bool {
        if self.ctx.in_try_block {
            return false;
        }

        if !e.may_have_side_effects() {
            return true;
        }

        match e {
            Expr::Ident(e) => {
                if let Some(a) = a {
                    match a {
                        Mergable::Var(a) => {
                            if is_ident_used_by(e.to_id(), &**a) {
                                return false;
                            }
                        }
                        Mergable::Expr(a) => {
                            if is_ident_used_by(e.to_id(), &**a) {
                                return false;
                            }
                        }
                    }
                }

                true
            }

            Expr::Lit(..) => true,
            Expr::Unary(UnaryExpr {
                op: op!("!") | op!("void") | op!("typeof"),
                arg,
                ..
            }) => self.is_skippable_for_seq(a, arg),

            Expr::Bin(BinExpr { left, right, .. }) => {
                self.is_skippable_for_seq(a, left) && self.is_skippable_for_seq(a, right)
            }

            Expr::Assign(e) => {
                let left_id = e.left.as_ident();
                let left_id = match left_id {
                    Some(v) => v,
                    _ => return false,
                };

                if let Some(a) = a {
                    match a {
                        Mergable::Var(a) => {
                            if is_ident_used_by(left_id.to_id(), &**a) {
                                return false;
                            }
                        }
                        Mergable::Expr(a) => {
                            if is_ident_used_by(left_id.to_id(), &**a) {
                                return false;
                            }
                        }
                    }
                }

                if let Expr::Lit(..) = &*e.right {
                    return true;
                }

                if contains_this_expr(&*e.right) {
                    return false;
                }

                let used_ids = idents_used_by(&*e.right);
                if used_ids.is_empty() {
                    return true;
                }

                if used_ids.len() != 1 || !used_ids.contains(&left_id.to_id()) {
                    return false;
                }

                self.is_skippable_for_seq(a, &e.right)
            }

            Expr::Object(e) => {
                if e.props.is_empty() {
                    return true;
                }

                // TODO: Check for side effects in object properties.

                false
            }

            Expr::Array(e) => {
                for elem in e.elems.iter().flatten() {
                    if !self.is_skippable_for_seq(a, &elem.expr) {
                        return false;
                    }
                }

                true
            }

            _ => false,
        }
    }

    /// Returns true if something is modified.
    ///
    /// Returns [Err] iff we should stop checking.
    fn merge_sequential_expr(&mut self, a: &mut Mergable, b: &mut Expr) -> Result<bool, ()> {
        let _tracing = if cfg!(feature = "debug") {
            let b_str = dump(&*b, false);
            let a_id = a.id();

            Some(
                span!(
                    Level::ERROR,
                    "merge_sequential_expr",
                    a_id = tracing::field::debug(&a_id),
                    b = &*b_str
                )
                .entered(),
            )
        } else {
            None
        };

        match a {
            Mergable::Var(..) => {}
            Mergable::Expr(a) => {
                if let Expr::Seq(a) = a {
                    //
                    for a in a.exprs.iter_mut().rev() {
                        if self.merge_sequential_expr(&mut Mergable::Expr(a), b)? {
                            return Ok(true);
                        }

                        if !self.is_skippable_for_seq(None, a) {
                            return Ok(false);
                        }
                    }

                    return Ok(false);
                }
            }
        }

        match b {
            Expr::Update(..) | Expr::Arrow(..) | Expr::Fn(..) => return Ok(false),

            Expr::Cond(b) => {
                tracing::trace!("seq: Try test of cond");
                return self.merge_sequential_expr(a, &mut *b.test);
            }

            Expr::Unary(b) => {
                tracing::trace!("seq: Try arg of unary");
                return self.merge_sequential_expr(a, &mut b.arg);
            }

            Expr::Bin(BinExpr {
                op, left, right, ..
            }) => {
                tracing::trace!("seq: Try left of bin");
                if self.merge_sequential_expr(a, &mut **left)? {
                    return Ok(true);
                }

                if !self.is_skippable_for_seq(Some(a), left) {
                    return Ok(false);
                }

                match *op {
                    op!("&&") | op!("||") | op!("??") => return Ok(false),
                    _ => {}
                }

                tracing::trace!("seq: Try right of bin");
                return self.merge_sequential_expr(a, &mut **right);
            }

            Expr::Member(MemberExpr { obj, prop, .. }) if !prop.is_computed() => {
                tracing::trace!("seq: Try object of member");
                return self.merge_sequential_expr(a, &mut **obj);
            }

            Expr::Member(MemberExpr {
                obj,
                prop: MemberProp::Computed(c),
                ..
            }) => {
                tracing::trace!("seq: Try object of member (computed)");
                if self.merge_sequential_expr(a, &mut **obj)? {
                    return Ok(true);
                }

                if obj.may_have_side_effects() {
                    return Ok(false);
                }

                tracing::trace!("seq: Try prop of member (computed)");
                return self.merge_sequential_expr(a, &mut c.expr);
            }

            Expr::SuperProp(SuperPropExpr {
                prop: SuperProp::Computed(c),
                ..
            }) => {
                tracing::trace!("seq: Try prop of member (computed)");
                return self.merge_sequential_expr(a, &mut c.expr);
            }

            Expr::Assign(b @ AssignExpr { op: op!("="), .. }) => {
                match &mut b.left {
                    PatOrExpr::Expr(b_left) => {
                        tracing::trace!("seq: Try lhs of assign");
                        if self.merge_sequential_expr(a, &mut **b_left)? {
                            return Ok(true);
                        }

                        match &**b_left {
                            Expr::Ident(..) => {}

                            _ => {
                                return Ok(false);
                            }
                        }
                    }
                    PatOrExpr::Pat(b_left) => match &mut **b_left {
                        Pat::Expr(b_left) => {
                            tracing::trace!("seq: Try lhs of assign");
                            if self.merge_sequential_expr(a, &mut **b_left)? {
                                return Ok(true);
                            }

                            match &**b_left {
                                Expr::Ident(..) => {}
                                _ => {
                                    return Ok(false);
                                }
                            }
                        }
                        Pat::Ident(..) => {}
                        _ => return Ok(false),
                    },
                }

                if should_not_check_rhs_of_assign(a, b) {
                    return Ok(false);
                }

                tracing::trace!("seq: Try rhs of assign");
                return self.merge_sequential_expr(a, &mut b.right);
            }

            Expr::Assign(b) => {
                if should_not_check_rhs_of_assign(a, b) {
                    return Ok(false);
                }

                let b_left = b.left.as_ident();
                let b_left = match b_left {
                    Some(v) => v.clone(),
                    None => return Ok(false),
                };

                if UsageFinder::find(&b_left, &b.right) {
                    return Err(());
                }

                tracing::trace!("seq: Try rhs of assign with op");
                return self.merge_sequential_expr(a, &mut b.right);
            }

            Expr::Array(b) => {
                for elem in &mut b.elems {
                    match elem {
                        Some(elem) => {
                            tracing::trace!("seq: Try element of array");
                            if self.merge_sequential_expr(a, &mut elem.expr)? {
                                return Ok(true);
                            }

                            if !self.is_skippable_for_seq(Some(a), &elem.expr) {
                                // To preserve side-effects, we need to abort.
                                break;
                            }
                        }
                        None => {}
                    }
                }

                return Ok(false);
            }

            Expr::Call(CallExpr {
                callee: Callee::Expr(b_callee),
                args: b_args,
                ..
            }) => {
                let is_this_undefined = b_callee.is_ident();
                tracing::trace!("seq: Try callee of call");
                if self.merge_sequential_expr(a, &mut **b_callee)? {
                    if is_this_undefined {
                        if let Expr::Member(..) = &**b_callee {
                            let zero = Box::new(Expr::Lit(Lit::Num(Number {
                                span: DUMMY_SP,
                                value: 0.0,
                            })));
                            tracing::debug!("injecting zero to preserve `this` in call");

                            *b_callee = Box::new(Expr::Seq(SeqExpr {
                                span: b_callee.span(),
                                exprs: vec![zero, b_callee.take()],
                            }));
                        }
                    }

                    return Ok(true);
                }

                if !self.is_skippable_for_seq(Some(a), b_callee) {
                    return Ok(false);
                }

                for arg in b_args {
                    tracing::trace!("seq: Try arg of call");
                    if self.merge_sequential_expr(a, &mut arg.expr)? {
                        return Ok(true);
                    }

                    if !self.is_skippable_for_seq(Some(a), &arg.expr) {
                        return Ok(false);
                    }
                }

                return Ok(false);
            }

            Expr::New(NewExpr {
                callee: b_callee, ..
            }) => {
                tracing::trace!("seq: Try callee of new");
                if self.merge_sequential_expr(a, &mut **b_callee)? {
                    return Ok(true);
                }

                return Ok(false);
            }

            Expr::Seq(SeqExpr { exprs: b_exprs, .. }) => {
                for b_expr in b_exprs {
                    tracing::trace!("seq: Try elem of seq");

                    if self.merge_sequential_expr(a, &mut **b_expr)? {
                        return Ok(true);
                    }

                    if !self.is_skippable_for_seq(Some(a), b_expr) {
                        return Ok(false);
                    }
                }

                return Ok(false);
            }

            Expr::Object(ObjectLit { props, .. }) => {
                for prop in props {
                    match prop {
                        PropOrSpread::Spread(prop) => {
                            if self.merge_sequential_expr(a, &mut *prop.expr)? {
                                return Ok(true);
                            }

                            return Ok(false);
                        }
                        PropOrSpread::Prop(prop) => {
                            // Inline into key
                            let key = match &mut **prop {
                                Prop::Shorthand(_) => continue,
                                Prop::KeyValue(prop) => Some(&mut prop.key),
                                Prop::Assign(_) => None,
                                Prop::Getter(prop) => Some(&mut prop.key),
                                Prop::Setter(prop) => Some(&mut prop.key),
                                Prop::Method(prop) => Some(&mut prop.key),
                            };

                            if let Some(PropName::Computed(key)) = key {
                                if self.merge_sequential_expr(a, &mut key.expr)? {
                                    return Ok(true);
                                }

                                if !self.is_skippable_for_seq(Some(a), &key.expr) {
                                    return Ok(false);
                                }
                            }

                            match &mut **prop {
                                Prop::KeyValue(prop) => {
                                    if self.merge_sequential_expr(a, &mut prop.value)? {
                                        return Ok(true);
                                    }

                                    if !self.is_skippable_for_seq(Some(a), &prop.value) {
                                        return Ok(false);
                                    }
                                }
                                Prop::Assign(prop) => {
                                    if self.merge_sequential_expr(a, &mut prop.value)? {
                                        return Ok(true);
                                    }

                                    if !self.is_skippable_for_seq(Some(a), &prop.value) {
                                        return Ok(false);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }

                return Ok(false);
            }

            _ => {}
        }

        // if cfg!(feature = "debug") && false {
        //     tracing::trace!(
        //         "sequences: Trying to merge `{}` => `{}`",
        //         dump(&*a),
        //         dump(&*b)
        //     );
        // }

        {
            // This requires tracking if `b` is in an assignment pattern.
            //
            // Update expressions can be inline.
            //
            // ++c, console.log(c)
            //
            // is same as
            //
            // console.log(c++)

            match a {
                Mergable::Var(_) => {}
                Mergable::Expr(a) => {
                    if let Expr::Update(UpdateExpr {
                        op,
                        prefix: false,
                        arg,
                        ..
                    }) = *a
                    {
                        if let Expr::Ident(a_id) = &**arg {
                            let mut v = UsageCounter {
                                expr_usage: Default::default(),
                                pat_usage: Default::default(),
                                target: &*a_id,
                                in_lhs: false,
                            };
                            b.visit_with(&mut v);
                            if v.expr_usage != 1 || v.pat_usage != 0 {
                                tracing::trace!(
                                    "[X] sequences: Aborting merging of an update expression \
                                     because of usage counts ({}, ref = {}, pat = {})",
                                    a_id,
                                    v.expr_usage,
                                    v.pat_usage
                                );
                                return Ok(false);
                            }

                            let mut replaced = false;
                            replace_expr(b, |e| {
                                if let Expr::Update(e @ UpdateExpr { prefix: false, .. }) = e {
                                    if *op == e.op && arg.is_ident_ref_to(a_id.sym.clone()) {
                                        e.prefix = true;
                                        replaced = true;
                                    }
                                }
                            });
                            if replaced {
                                a.take();
                                return Ok(true);
                            }
                        }

                        return Ok(false);
                    }
                }
            }
        }

        let mut right_val;
        let (left_id, right) = match a {
            Mergable::Expr(a) => {
                match a {
                    Expr::Assign(AssignExpr { left, right, .. }) => {
                        // (a = 5, console.log(a))
                        //
                        // =>
                        //
                        // (console.log(a = 5))

                        let left_id = match left.as_ident() {
                            Some(v) => v,
                            None => {
                                tracing::trace!("[X] sequences: Aborting because lhs is not an id");
                                return Ok(false);
                            }
                        };

                        if let Some(usage) = self
                            .data
                            .as_ref()
                            .and_then(|data| data.vars.get(&left_id.to_id()))
                        {
                            if usage.inline_prevented {
                                return Ok(false);
                            }

                            // Reassignment to const?
                            if let Some(VarDeclKind::Const) = usage.var_kind {
                                return Ok(false);
                            }

                            if usage.declared_as_fn_expr {
                                tracing::trace!(
                                    "sequences: [X] Declared as fn expr ({}, {:?})",
                                    left_id.sym,
                                    left_id.span.ctxt
                                );
                                return Ok(false);
                            }
                        }

                        (left_id.clone(), right)
                    }
                    _ => return Ok(false),
                }
            }

            Mergable::Var(a) => {
                let left = match &a.name {
                    Pat::Ident(i) => i.id.clone(),
                    _ => return Ok(false),
                };

                if let Some(usage) = self
                    .data
                    .as_ref()
                    .and_then(|data| data.vars.get(&left.to_id()))
                {
                    if usage.ref_count != 1 {
                        return Ok(false);
                    }
                    if usage.reassigned() || !usage.is_fn_local {
                        return Ok(false);
                    }
                    if usage.inline_prevented {
                        return Ok(false);
                    }

                    match &mut a.init {
                        Some(v) => (left, v),
                        None => {
                            if usage.declared_count > 1 {
                                return Ok(false);
                            }

                            right_val = undefined(DUMMY_SP);
                            (left, &mut right_val)
                        }
                    }
                } else {
                    return Ok(false);
                }
            }
        };

        if right.is_this() || right.is_ident_ref_to(js_word!("arguments")) {
            return Ok(false);
        }
        if contains_arguments(&**right) {
            return Ok(false);
        }

        {
            // Abort this if there's some side effects.
            //
            //
            // (rand = _.random(
            //     index++
            // )),
            // (shuffled[index - 1] = shuffled[rand]),
            // (shuffled[rand] = value);
            //
            //
            // rand should not be inlined because of `index`.

            let deps = idents_used_by_ignoring_nested(&*right);

            let used_by_b = idents_used_by(&*b);

            for id in &deps {
                if *id == left_id.to_id() {
                    continue;
                }

                if used_by_b.contains(id) {
                    tracing::trace!("[X] sequences: Aborting because of deps");
                    return Err(());
                }
            }
        }

        {
            let mut v = UsageCounter {
                expr_usage: Default::default(),
                pat_usage: Default::default(),
                target: &left_id,
                in_lhs: false,
            };
            b.visit_with(&mut v);
            if v.expr_usage != 1 || v.pat_usage != 0 {
                if cfg!(feature = "debug") {
                    tracing::trace!(
                        "[X] sequences: Aborting because of usage counts ({}{:?}, ref = {}, pat = \
                         {})",
                        left_id.sym,
                        left_id.span.ctxt,
                        v.expr_usage,
                        v.pat_usage
                    );
                }
                return Ok(false);
            }
        }

        self.changed = true;
        tracing::debug!(
            "sequences: Inlining sequential expressions (`{}{:?}`)",
            left_id.sym,
            left_id.span.ctxt
        );

        let to = match a {
            Mergable::Var(a) => a.init.take().unwrap_or_else(|| undefined(DUMMY_SP)),
            Mergable::Expr(a) => Box::new(a.take()),
        };

        replace_id_with_expr(b, left_id.to_id(), to);

        if cfg!(feature = "debug") {
            tracing::debug!("sequences: [Changed] {}", dump(&*b, false));
        }

        Ok(true)
    }
}

/// TODO(kdy1): Optimize this
///
/// See https://github.com/swc-project/swc/pull/3480
///
/// This works, but it should be optimized.
///
/// This check blocks optimization of clearly valid optimizations like `i += 1,
/// arr[i]`
fn should_not_check_rhs_of_assign(a: &Mergable, b: &mut AssignExpr) -> bool {
    if let Some(a_id) = a.id() {
        match a {
            Mergable::Expr(Expr::Assign(AssignExpr { op: op!("="), .. })) => {}
            Mergable::Expr(Expr::Assign(..)) => {
                let used_by_b = idents_used_by(&*b.right);
                if used_by_b.contains(&a_id) {
                    return true;
                }
            }
            _ => {}
        }
    }

    false
}

struct UsageCounter<'a> {
    expr_usage: usize,
    pat_usage: usize,

    target: &'a Ident,
    in_lhs: bool,
}

impl Visit for UsageCounter<'_> {
    noop_visit_type!();

    fn visit_ident(&mut self, i: &Ident) {
        if self.target.sym == i.sym && self.target.span.ctxt == i.span.ctxt {
            if self.in_lhs {
                self.pat_usage += 1;
            } else {
                self.expr_usage += 1;
            }
        }
    }

    fn visit_member_expr(&mut self, e: &MemberExpr) {
        e.obj.visit_with(self);

        if let MemberProp::Computed(c) = &e.prop {
            let old = self.in_lhs;
            self.in_lhs = false;
            c.expr.visit_with(self);
            self.in_lhs = old;
        }
    }

    fn visit_super_prop_expr(&mut self, e: &SuperPropExpr) {
        if let SuperProp::Computed(c) = &e.prop {
            let old = self.in_lhs;
            self.in_lhs = false;
            c.expr.visit_with(self);
            self.in_lhs = old;
        }
    }

    fn visit_pat(&mut self, p: &Pat) {
        let old = self.in_lhs;
        self.in_lhs = true;
        p.visit_children_with(self);
        self.in_lhs = old;
    }

    fn visit_pat_or_expr(&mut self, p: &PatOrExpr) {
        let old = self.in_lhs;
        self.in_lhs = true;
        p.visit_children_with(self);
        self.in_lhs = old;
    }
}

enum Mergable<'a> {
    Var(&'a mut VarDeclarator),
    Expr(&'a mut Expr),
}

impl Mergable<'_> {
    fn id(&self) -> Option<Id> {
        match self {
            Mergable::Var(s) => match &s.name {
                Pat::Ident(i) => Some(i.id.to_id()),
                _ => None,
            },
            Mergable::Expr(s) => match &**s {
                Expr::Assign(s) => s.left.as_ident().map(|v| v.to_id()),
                _ => None,
            },
        }
    }
}
