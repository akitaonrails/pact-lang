use crate::ast::*;
use crate::diagnostics::Diagnostic;

/// Check match exhaustiveness for functions marked as total.
/// For the prototype, we check that match expressions on union return types
/// cover all declared variants (ok + all err tags).
pub fn check_totality(module: &Module) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for func in &module.functions {
        if !func.total {
            continue;
        }

        // Collect the expected return variants
        let expected_tags: Vec<String> = func
            .returns
            .variants
            .iter()
            .map(|v| match &v.kind {
                VariantKind::Ok { .. } => "ok".to_string(),
                VariantKind::Err { tag, .. } => tag.clone(),
            })
            .collect();

        // Check the body expression for exhaustiveness
        check_expr_totality(&func.body, &expected_tags, &func.name, &mut diagnostics);
    }

    diagnostics
}

fn check_expr_totality(
    expr: &Expr,
    _expected_return_tags: &[String],
    fn_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expr {
        Expr::Match { arms, span, .. } => {
            // Check if match has a wildcard/catch-all arm
            let has_wildcard = arms.iter().any(|arm| is_catch_all(&arm.pattern));

            if !has_wildcard {
                // Collect matched patterns
                let matched_tags: Vec<String> = arms
                    .iter()
                    .filter_map(|arm| pattern_tag(&arm.pattern))
                    .collect();

                // For result-type matches (ok/err), check coverage
                let has_ok = matched_tags.iter().any(|t| t == "ok");
                let has_err = matched_tags.iter().any(|t| t == "err" || t != "ok");

                if !has_ok && !has_err && !matched_tags.is_empty() {
                    diagnostics.push(Diagnostic::warning(
                        format!(
                            "match in '{}' may not be exhaustive: matched {:?}",
                            fn_name, matched_tags
                        ),
                        Some(span.clone()),
                    ));
                }
            }

            // Recurse into arm bodies
            for arm in arms {
                check_expr_totality(&arm.body, _expected_return_tags, fn_name, diagnostics);
            }
        }
        Expr::Let { body, bindings, .. } => {
            for (_, val) in bindings {
                check_expr_totality(val, _expected_return_tags, fn_name, diagnostics);
            }
            check_expr_totality(body, _expected_return_tags, fn_name, diagnostics);
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            check_expr_totality(cond, _expected_return_tags, fn_name, diagnostics);
            check_expr_totality(then_branch, _expected_return_tags, fn_name, diagnostics);
            check_expr_totality(else_branch, _expected_return_tags, fn_name, diagnostics);
        }
        Expr::Call { args, .. } => {
            for arg in args {
                check_expr_totality(arg, _expected_return_tags, fn_name, diagnostics);
            }
        }
        Expr::Ok(inner, _) => {
            check_expr_totality(inner, _expected_return_tags, fn_name, diagnostics);
        }
        Expr::Err { payload, .. } => {
            check_expr_totality(payload, _expected_return_tags, fn_name, diagnostics);
        }
        Expr::FieldAccess { expr, .. } => {
            check_expr_totality(expr, _expected_return_tags, fn_name, diagnostics);
        }
        Expr::MapLit(entries, _) => {
            for (_, val) in entries {
                check_expr_totality(val, _expected_return_tags, fn_name, diagnostics);
            }
        }
        _ => {}
    }
}

fn is_catch_all(pattern: &Pattern) -> bool {
    match pattern {
        Pattern::Wildcard(_) => true,
        Pattern::Var(_, _) => true,
        _ => false,
    }
}

fn pattern_tag(pattern: &Pattern) -> Option<String> {
    match pattern {
        Pattern::Constructor { name, .. } => Some(name.clone()),
        Pattern::Keyword(kw, _) => Some(kw.clone()),
        _ => None,
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
        check_totality(&module)
    }

    #[test]
    fn test_exhaustive_match() {
        let input = r#"(module test :version 1
            (fn get-thing
                :effects []
                :total true
                (param id UUID)
                (returns (union
                    (ok UUID :http 200)
                    (err :not-found {:id id} :http 404)))
                (match id
                    (ok x)  (ok x)
                    (err _) (err :not-found {:id id}))))"#;
        let diags = check(input);
        let errors: Vec<_> = diags
            .iter()
            .filter(|d| d.kind == crate::diagnostics::DiagnosticKind::Error)
            .collect();
        assert!(errors.is_empty());
    }

    #[test]
    fn test_total_with_wildcard() {
        let input = r#"(module test :version 1
            (fn get-thing
                :effects []
                :total true
                (param id UUID)
                (returns (union (ok UUID :http 200)))
                (match id
                    _ (ok id))))"#;
        let diags = check(input);
        let errors: Vec<_> = diags
            .iter()
            .filter(|d| d.kind == crate::diagnostics::DiagnosticKind::Error)
            .collect();
        assert!(errors.is_empty());
    }
}
