use super::Simplify;
use ast::*;
use ast::{Ident, Lit};
use ast::ExprKind::*;
use swc_common::fold::{FoldWith, Folder};
use util::*;

impl Folder<ExprKind> for Simplify {
    fn fold(&mut self, expr: ExprKind) -> ExprKind {
        // fold children nodes.
        let expr = expr.fold_children(self);

        match expr {
            // Do nothing for literals.
            Lit(_) => expr,

            Unary { prefix, op, arg } => fold_unary(prefix, op, arg),
            Binary { left, op, right } => fold_bin(left, op, right),

            Cond { test, cons, alt } => match test.as_bool() {
                (p, Known(val)) => {
                    let expr_value = if val { cons } else { alt };
                    if p.is_pure() {
                        expr_value.node
                    } else {
                        Seq {
                            exprs: vec![test, expr_value],
                        }
                    }
                }
                _ => Cond { test, cons, alt },
            },

            // Simplify sequence expression.
            Seq { exprs } => if exprs.len() == 1 {
                exprs.into_iter().next().unwrap().node
            } else {
                assert!(!exprs.is_empty(), "sequence expression should not be empty");
                //TODO: remove unused
                return Seq { exprs };
            },

            // be conservative.
            _ => expr,
        }
    }
}

fn fold_bin(left: Box<Expr>, op: BinaryOp, right: Box<Expr>) -> ExprKind {
    let (left, right) = match op {
        op!(bin "+") => return fold_add(left, right),
        op!("&&") | op!("||") => match left.as_bool() {
            (Pure, Known(val)) => {
                if op == op!("&&") {
                    if val {
                        // 1 && $right
                        return right.node;
                    } else {
                        // 0 && $right
                        return Lit(Lit::Bool(false));
                    }
                } else {
                    if val {
                        // 1 || $right
                        return left.node;
                    } else {
                        // 0 || $right
                        return right.node;
                    }
                }
            }
            _ => (left, right),
        },
        op!("instanceof") => (left, right),

        op!(bin "-") | op!("/") | op!("%") => {
            // Arithmetic operations

            (left, right)
        }
        _ => (left, right),
    };

    Binary { left, op, right }
}

///See https://tc39.github.io/ecma262/#sec-addition-operator-plus
fn fold_add(left: Box<Expr>, right: Box<Expr>) -> ExprKind {
    // It's string concatenation if either left or right is string.

    Binary {
        left,
        op: op!(bin "+"),
        right,
    }
}

fn fold_unary(prefix: bool, op: UnaryOp, arg: Box<Expr>) -> ExprKind {
    let span = arg.span;

    match op {
        op!("typeof") => {
            let val = match arg.node {
                Function(..) => "function",
                Lit(Lit::Str(..)) => "string",
                Lit(Lit::Num(..)) => "number",
                Lit(Lit::Bool(..)) => "boolean",
                Lit(Lit::Null) | Object { .. } | Array { .. } => "object",
                Unary {
                    prefix: true,
                    op: op!("void"),
                    ..
                }
                | Ident(Ident {
                    sym: js_word!("undefined"),
                    ..
                }) => {
                    // We can assume `undefined` is `undefined`,
                    // because overriding `undefined` is always hard error in swc.
                    "undefined"
                }

                _ => {
                    return Unary {
                        prefix: true,
                        op: op!("typeof"),
                        arg,
                    }
                }
            };

            return Lit(Lit::Str(val.into()));
        }
        op!("!") => match arg.as_bool() {
            (p, Known(val)) => {
                let new = Lit(Lit::Bool(!val));
                return if p.is_pure() {
                    new
                } else {
                    Seq {
                        exprs: vec![arg, box Expr { span, node: new }],
                    }
                };
            }
            _ => {
                return Unary {
                    op,
                    arg,
                    prefix: true,
                }
            }
        },
        op!(unary "+") => {}
        op!(unary "-") => match arg.node {
            Ident(Ident {
                sym: js_word!("Infinity"),
                ..
            }) => return Unary { prefix, op, arg },
            // "-NaN" is "NaN"
            Ident(Ident {
                sym: js_word!("NaN"),
                ..
            }) => return arg.node,
            Lit(Lit::Num(Number(f))) => return Lit(Lit::Num(Number(-f))),
            _ => {

                // TODO: Report that user is something bad (negating non-number value)
            }
        },
        _ => {}
    }

    Unary { prefix, op, arg }
}

/// Try to fold arithmetic binary operators
fn perform_arithmetic_op(op: BinaryOp, left: Box<Expr>, right: Box<Expr>) -> ExprKind {
    let (lv, rv) = (left.as_number(), right.as_number());

    if lv.is_unknown() && rv.is_unknown() {
        return Binary { left, op, right };
    }

    Binary { left, op, right }
}

/// https://tc39.github.io/ecma262/#sec-abstract-equality-comparison
fn perform_abstract_eq_cmp(left: &Expr, right: &Expr) -> Value<bool> {}

/// https://tc39.github.io/ecma262/#sec-strict-equality-comparison
fn perform_strict_eq_cmp(left: &Expr, right: &Expr) -> Value<bool> {}
