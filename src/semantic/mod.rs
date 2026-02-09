pub mod resolve;
pub mod effects;
pub mod totality;

use crate::ast::Module;
use crate::diagnostics::Diagnostic;

/// Run all semantic analysis passes on a module.
/// Returns a list of diagnostics (errors and warnings).
pub fn analyze(module: &Module) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    diagnostics.extend(resolve::resolve_names(module));
    diagnostics.extend(effects::check_effects(module));
    diagnostics.extend(totality::check_totality(module));

    diagnostics
}
