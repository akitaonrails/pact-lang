use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::diagnostics::Diagnostic;

/// Check that function bodies only use effects declared in their effect annotations.
/// For the prototype, we check that function calls to store operations (query, insert!, etc.)
/// only happen in functions that declare the appropriate effect sets.
pub fn check_effects(module: &Module) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    // Build effect set map: name -> set of (kind, target)
    let mut effect_map: HashMap<String, HashSet<(EffectKind, String)>> = HashMap::new();
    for es in &module.effect_sets {
        let mut set = HashSet::new();
        for eff in &es.effects {
            set.insert((eff.kind.clone(), eff.target.clone()));
        }
        effect_map.insert(es.name.clone(), set);
    }

    for func in &module.functions {
        // Collect all effects this function is allowed to use
        let mut allowed_effects: HashSet<(EffectKind, String)> = HashSet::new();
        for effect_name in &func.effects {
            if let Some(effects) = effect_map.get(effect_name) {
                allowed_effects.extend(effects.iter().cloned());
            }
        }

        // Check the body for effectful operations
        let used_effects = collect_used_effects(&func.body);

        for (kind, target) in &used_effects {
            if !allowed_effects.contains(&(kind.clone(), target.clone())) {
                diagnostics.push(Diagnostic::error(
                    format!(
                        "function '{}' performs {:?} on '{}' but does not declare that effect",
                        func.name, kind, target
                    ),
                    Some(func.span.clone()),
                ));
            }
        }
    }

    diagnostics
}

/// Collect effects used in an expression.
/// For the prototype, we recognize patterns like:
/// - (query store-name ...) → Reads on store-name
/// - (insert! store-name ...) → Writes on store-name
fn collect_used_effects(expr: &Expr) -> HashSet<(EffectKind, String)> {
    let mut effects = HashSet::new();
    collect_effects_inner(expr, &mut effects);
    effects
}

fn collect_effects_inner(expr: &Expr, effects: &mut HashSet<(EffectKind, String)>) {
    match expr {
        Expr::Call { name, args, .. } => {
            // Recognize effectful operations
            match name.as_str() {
                "query" | "get" | "lookup" => {
                    if let Some(Expr::Ref(target, _)) = args.first() {
                        effects.insert((EffectKind::Reads, target.clone()));
                    }
                }
                n if n.ends_with('!') => {
                    // Convention: functions ending with ! are write operations
                    // First arg is typically the store
                    if let Some(Expr::Ref(target, _)) = args.first() {
                        effects.insert((EffectKind::Writes, target.clone()));
                    }
                }
                _ => {}
            }
            for arg in args {
                collect_effects_inner(arg, effects);
            }
        }
        Expr::Let {
            bindings, body, ..
        } => {
            for (_, value) in bindings {
                collect_effects_inner(value, effects);
            }
            collect_effects_inner(body, effects);
        }
        Expr::Match { expr, arms, .. } => {
            collect_effects_inner(expr, effects);
            for arm in arms {
                collect_effects_inner(&arm.body, effects);
            }
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            collect_effects_inner(cond, effects);
            collect_effects_inner(then_branch, effects);
            collect_effects_inner(else_branch, effects);
        }
        Expr::FieldAccess { expr, .. } => {
            collect_effects_inner(expr, effects);
        }
        Expr::Ok(inner, _) => {
            collect_effects_inner(inner, effects);
        }
        Expr::Err { payload, .. } => {
            collect_effects_inner(payload, effects);
        }
        Expr::MapLit(entries, _) => {
            for (_, val) in entries {
                collect_effects_inner(val, effects);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::lower::Lowerer;
    use crate::parser::Parser;

    fn check(input: &str) -> Vec<Diagnostic> {
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let sexprs = parser.parse_program().unwrap();
        let mut lowerer = Lowerer::new();
        let module = lowerer.lower_module(&sexprs[0]).unwrap();
        check_effects(&module)
    }

    #[test]
    fn test_effects_ok() {
        let input = r#"(module test :version 1
            (effect-set db-read [:reads user-store])
            (fn get-thing
                :effects [db-read]
                :total true
                (param id UUID)
                (returns (union (ok UUID :http 200)))
                (query user-store {:id id})))"#;
        let diags = check(input);
        let errors: Vec<_> = diags
            .iter()
            .filter(|d| d.kind == crate::diagnostics::DiagnosticKind::Error)
            .collect();
        assert!(errors.is_empty(), "expected no errors, got: {:?}", errors);
    }

    #[test]
    fn test_effects_missing() {
        let input = r#"(module test :version 1
            (effect-set db-read [:reads user-store])
            (fn get-thing
                :effects []
                :total true
                (param id UUID)
                (returns (union (ok UUID :http 200)))
                (query user-store {:id id})))"#;
        let diags = check(input);
        let errors: Vec<_> = diags
            .iter()
            .filter(|d| d.kind == crate::diagnostics::DiagnosticKind::Error)
            .collect();
        assert!(!errors.is_empty(), "expected effect error");
        assert!(errors[0].message.contains("Reads"));
    }
}
