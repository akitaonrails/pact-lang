use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::diagnostics::Diagnostic;
use crate::lexer::Span;

/// Symbol table for name resolution
pub struct SymbolTable {
    pub types: HashMap<String, TypeInfo>,
    pub effect_sets: HashMap<String, EffectSetInfo>,
    pub functions: HashMap<String, FnInfo>,
    pub stores: HashSet<String>,
}

#[derive(Debug)]
pub struct TypeInfo {
    pub fields: Vec<String>,
    pub span: Span,
}

#[derive(Debug)]
pub struct EffectSetInfo {
    pub effects: Vec<Effect>,
    pub span: Span,
}

#[derive(Debug)]
pub struct FnInfo {
    pub effect_names: Vec<String>,
    pub param_names: Vec<String>,
    pub return_variants: Vec<String>,
    pub span: Span,
}

pub fn resolve_names(module: &Module) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut symtab = SymbolTable {
        types: HashMap::new(),
        effect_sets: HashMap::new(),
        functions: HashMap::new(),
        stores: HashSet::new(),
    };

    // First pass: register all declarations
    for typedef in &module.types {
        let fields: Vec<String> = typedef.fields.iter().map(|f| f.name.clone()).collect();
        symtab.types.insert(
            typedef.name.clone(),
            TypeInfo {
                fields,
                span: typedef.span.clone(),
            },
        );
    }

    for effect_set in &module.effect_sets {
        // Collect store names
        for eff in &effect_set.effects {
            symtab.stores.insert(eff.target.clone());
        }
        symtab.effect_sets.insert(
            effect_set.name.clone(),
            EffectSetInfo {
                effects: effect_set.effects.clone(),
                span: effect_set.span.clone(),
            },
        );
    }

    for func in &module.functions {
        let param_names: Vec<String> = func.params.iter().map(|p| p.name.clone()).collect();
        let return_variants: Vec<String> = func
            .returns
            .variants
            .iter()
            .map(|v| match &v.kind {
                VariantKind::Ok { .. } => "ok".to_string(),
                VariantKind::Err { tag, .. } => tag.clone(),
            })
            .collect();
        symtab.functions.insert(
            func.name.clone(),
            FnInfo {
                effect_names: func.effects.clone(),
                param_names,
                return_variants,
                span: func.span.clone(),
            },
        );
    }

    // Second pass: check references
    for func in &module.functions {
        // Check effect set references
        for effect_name in &func.effects {
            if !symtab.effect_sets.contains_key(effect_name) {
                diagnostics.push(Diagnostic::error(
                    format!(
                        "function '{}' references unknown effect set '{}'",
                        func.name, effect_name
                    ),
                    Some(func.span.clone()),
                ));
            }
        }

        // Check param type references
        for param in &func.params {
            check_type_ref(&param.type_expr, &symtab, &func.name, &mut diagnostics);
        }

        // Check return type references
        for variant in &func.returns.variants {
            match &variant.kind {
                VariantKind::Ok { type_expr, .. } => {
                    check_type_ref(type_expr, &symtab, &func.name, &mut diagnostics);
                }
                VariantKind::Err { payload, .. } => {
                    check_type_ref(payload, &symtab, &func.name, &mut diagnostics);
                }
            }
        }

        // Check body references
        let mut scope: HashSet<String> = func.params.iter().map(|p| p.name.clone()).collect();
        check_expr_refs(&func.body, &symtab, &mut scope, &func.name, &mut diagnostics);
    }

    diagnostics
}

fn check_type_ref(
    type_expr: &TypeExpr,
    symtab: &SymbolTable,
    context: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match type_expr {
        TypeExpr::Named(name) => {
            // Built-in types
            let builtins = [
                "UUID", "String", "Int", "Bool", "Unit", "ValidationError",
            ];
            if !builtins.contains(&name.as_str()) && !symtab.types.contains_key(name) {
                diagnostics.push(Diagnostic::warning(
                    format!(
                        "in '{}': type '{}' is not defined in this module",
                        context, name
                    ),
                    None,
                ));
            }
        }
        TypeExpr::Map(fields) => {
            for (_, typ) in fields {
                check_type_ref(typ, symtab, context, diagnostics);
            }
        }
        TypeExpr::List(inner) => {
            check_type_ref(inner, symtab, context, diagnostics);
        }
        TypeExpr::Union(variants) => {
            for v in variants {
                match &v.kind {
                    VariantKind::Ok { type_expr, .. } => {
                        check_type_ref(type_expr, symtab, context, diagnostics);
                    }
                    VariantKind::Err { payload, .. } => {
                        check_type_ref(payload, symtab, context, diagnostics);
                    }
                }
            }
        }
        TypeExpr::Enum(_) => {}
    }
}

fn check_expr_refs(
    expr: &Expr,
    symtab: &SymbolTable,
    scope: &mut HashSet<String>,
    context: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expr {
        Expr::Let {
            bindings, body, ..
        } => {
            for (name, value) in bindings {
                check_expr_refs(value, symtab, scope, context, diagnostics);
                scope.insert(name.clone());
            }
            check_expr_refs(body, symtab, scope, context, diagnostics);
        }
        Expr::Match { expr, arms, .. } => {
            check_expr_refs(expr, symtab, scope, context, diagnostics);
            for arm in arms {
                let mut arm_scope = scope.clone();
                collect_pattern_bindings(&arm.pattern, &mut arm_scope);
                check_expr_refs(&arm.body, symtab, &mut arm_scope, context, diagnostics);
            }
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            check_expr_refs(cond, symtab, scope, context, diagnostics);
            check_expr_refs(then_branch, symtab, scope, context, diagnostics);
            check_expr_refs(else_branch, symtab, scope, context, diagnostics);
        }
        Expr::Call { args, .. } => {
            for arg in args {
                check_expr_refs(arg, symtab, scope, context, diagnostics);
            }
        }
        Expr::FieldAccess { expr, .. } => {
            check_expr_refs(expr, symtab, scope, context, diagnostics);
        }
        Expr::Ok(inner, _) => {
            check_expr_refs(inner, symtab, scope, context, diagnostics);
        }
        Expr::Err { payload, .. } => {
            check_expr_refs(payload, symtab, scope, context, diagnostics);
        }
        Expr::MapLit(entries, _) => {
            for (_, val) in entries {
                check_expr_refs(val, symtab, scope, context, diagnostics);
            }
        }
        Expr::Ref(_, _)
        | Expr::Keyword(_, _)
        | Expr::StringLit(_, _)
        | Expr::IntLit(_, _)
        | Expr::BoolLit(_, _)
        | Expr::Wildcard(_) => {}
    }
}

fn collect_pattern_bindings(pattern: &Pattern, scope: &mut HashSet<String>) {
    match pattern {
        Pattern::Var(name, _) => {
            scope.insert(name.clone());
        }
        Pattern::Constructor { args, .. } => {
            for arg in args {
                collect_pattern_bindings(arg, scope);
            }
        }
        Pattern::Wildcard(_) | Pattern::Keyword(_, _) => {}
    }
}
