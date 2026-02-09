use crate::ast::*;
use crate::diagnostics::Diagnostic;
use crate::lexer::Span;
use crate::parser::{AtomKind, SExpr, SExprKind};

pub struct Lowerer {
    pub diagnostics: Vec<Diagnostic>,
}

impl Lowerer {
    pub fn new() -> Self {
        Lowerer {
            diagnostics: Vec::new(),
        }
    }

    pub fn lower_module(&mut self, sexpr: &SExpr) -> Result<Module, String> {
        let items = sexpr
            .as_list()
            .ok_or_else(|| format!("expected module to be a list"))?;
        if items.is_empty() || items[0].as_symbol() != Some("module") {
            return Err("expected (module ...)".to_string());
        }
        let name = items
            .get(1)
            .and_then(|s| s.as_symbol())
            .ok_or_else(|| "expected module name".to_string())?
            .to_string();

        let mut provenance = None;
        let mut version = None;
        let mut parent_version = None;
        let mut delta = None;
        let mut types = Vec::new();
        let mut effect_sets = Vec::new();
        let mut functions = Vec::new();
        let mut extra_meta = Vec::new();

        let mut i = 2;
        while i < items.len() {
            if let Some(kw) = items[i].as_keyword() {
                match kw {
                    "provenance" => {
                        i += 1;
                        provenance = Some(self.lower_provenance(&items[i])?);
                    }
                    "version" => {
                        i += 1;
                        version = items[i].as_int();
                    }
                    "parent-version" => {
                        i += 1;
                        parent_version = items[i].as_int();
                    }
                    "delta" => {
                        i += 1;
                        delta = Some(self.lower_delta(&items[i])?);
                    }
                    other => {
                        i += 1;
                        if i < items.len() {
                            extra_meta.push((other.to_string(), self.sexpr_to_meta(&items[i])));
                        }
                    }
                }
                i += 1;
            } else if let Some(list) = items[i].as_list() {
                if let Some(head) = list.first().and_then(|s| s.as_symbol()) {
                    match head {
                        "type" => types.push(self.lower_type_def(&items[i])?),
                        "effect-set" => effect_sets.push(self.lower_effect_set(&items[i])?),
                        "fn" => functions.push(self.lower_fn_def(&items[i])?),
                        _ => {
                            self.diagnostics.push(Diagnostic::warning(
                                format!("unknown top-level form '{}'", head),
                                Some(items[i].span.clone()),
                            ));
                        }
                    }
                }
                i += 1;
            } else {
                i += 1;
            }
        }

        Ok(Module {
            name,
            provenance,
            version,
            parent_version,
            delta,
            types,
            effect_sets,
            functions,
            extra_meta,
            span: sexpr.span.clone(),
        })
    }

    fn lower_provenance(&mut self, sexpr: &SExpr) -> Result<Provenance, String> {
        let entries = sexpr
            .as_map()
            .ok_or_else(|| "expected provenance to be a map".to_string())?;

        let mut req = None;
        let mut author = None;
        let mut created = None;
        let mut test = Vec::new();
        let mut extra = Vec::new();

        for (key, value) in entries {
            let key_name = key
                .as_symbol()
                .or_else(|| key.as_keyword())
                .ok_or_else(|| "expected provenance key to be a symbol or keyword".to_string())?;
            match key_name {
                "req" => req = value.as_string().map(|s| s.to_string()),
                "author" => author = value.as_string().map(|s| s.to_string()),
                "created" => created = value.as_string().map(|s| s.to_string()),
                "test" => {
                    if let Some(items) = value.as_vector() {
                        for item in items {
                            if let Some(s) = item.as_string() {
                                test.push(s.to_string());
                            }
                        }
                    }
                }
                other => {
                    extra.push((other.to_string(), self.sexpr_to_meta(value)));
                }
            }
        }

        Ok(Provenance {
            req,
            author,
            created,
            test,
            extra,
            span: sexpr.span.clone(),
        })
    }

    fn lower_delta(&mut self, sexpr: &SExpr) -> Result<Delta, String> {
        let items = sexpr
            .as_list()
            .ok_or_else(|| "expected delta to be a list".to_string())?;
        let operation = items
            .first()
            .and_then(|s| s.as_symbol())
            .unwrap_or("unknown")
            .to_string();
        let target = items
            .get(1)
            .and_then(|s| s.as_symbol())
            .unwrap_or("")
            .to_string();
        let description = items.get(2).and_then(|s| s.as_string()).map(|s| s.to_string());

        Ok(Delta {
            operation,
            target,
            description,
            span: sexpr.span.clone(),
        })
    }

    fn lower_type_def(&mut self, sexpr: &SExpr) -> Result<TypeDef, String> {
        let items = sexpr.as_list().ok_or("expected type to be a list")?;
        // (type Name :invariants [...] (field ...) ...)
        let name = items
            .get(1)
            .and_then(|s| s.as_symbol())
            .ok_or("expected type name")?
            .to_string();

        let mut invariants = Vec::new();
        let mut fields = Vec::new();
        let mut extra_meta = Vec::new();

        let mut i = 2;
        while i < items.len() {
            if let Some(kw) = items[i].as_keyword() {
                match kw {
                    "invariants" => {
                        i += 1;
                        if let Some(inv_items) = items[i].as_vector() {
                            for inv in inv_items {
                                invariants.push(InvariantExpr {
                                    raw: format_sexpr(inv),
                                    span: inv.span.clone(),
                                });
                            }
                        }
                    }
                    other => {
                        i += 1;
                        if i < items.len() {
                            extra_meta.push((other.to_string(), self.sexpr_to_meta(&items[i])));
                        }
                    }
                }
                i += 1;
            } else if let Some(list) = items[i].as_list() {
                if list.first().and_then(|s| s.as_symbol()) == Some("field") {
                    fields.push(self.lower_field_def(&items[i])?);
                }
                i += 1;
            } else {
                i += 1;
            }
        }

        Ok(TypeDef {
            name,
            invariants,
            fields,
            extra_meta,
            span: sexpr.span.clone(),
        })
    }

    fn lower_field_def(&mut self, sexpr: &SExpr) -> Result<FieldDef, String> {
        let items = sexpr.as_list().ok_or("expected field to be a list")?;
        // (field name Type :keyword value ...)
        let name = items
            .get(1)
            .and_then(|s| s.as_symbol())
            .ok_or("expected field name")?
            .to_string();
        let type_expr = items
            .get(2)
            .map(|s| self.lower_type_expr(s))
            .ok_or("expected field type")??;

        let mut immutable = false;
        let mut generated = false;
        let mut min_len = None;
        let mut max_len = None;
        let mut format = None;
        let mut unique_within = None;
        let mut extra_meta = Vec::new();

        let mut i = 3;
        while i < items.len() {
            if let Some(kw) = items[i].as_keyword() {
                match kw {
                    "immutable" => {
                        immutable = true;
                        i += 1;
                    }
                    "generated" => {
                        generated = true;
                        i += 1;
                    }
                    "min-len" => {
                        i += 1;
                        min_len = items.get(i).and_then(|s| s.as_int());
                        i += 1;
                    }
                    "max-len" => {
                        i += 1;
                        max_len = items.get(i).and_then(|s| s.as_int());
                        i += 1;
                    }
                    "format" => {
                        i += 1;
                        format = items.get(i).and_then(|s| s.as_keyword()).map(|s| s.to_string());
                        i += 1;
                    }
                    "unique-within" => {
                        i += 1;
                        unique_within =
                            items.get(i).and_then(|s| s.as_symbol()).map(|s| s.to_string());
                        i += 1;
                    }
                    other => {
                        // Check if the next item is a value or another keyword (flag keyword)
                        if i + 1 < items.len() && items[i + 1].as_keyword().is_none() {
                            i += 1;
                            extra_meta.push((other.to_string(), self.sexpr_to_meta(&items[i])));
                            i += 1;
                        } else {
                            extra_meta
                                .push((other.to_string(), MetaValue::Bool(true)));
                            i += 1;
                        }
                    }
                }
            } else {
                i += 1;
            }
        }

        Ok(FieldDef {
            name,
            type_expr,
            immutable,
            generated,
            min_len,
            max_len,
            format,
            unique_within,
            extra_meta,
            span: sexpr.span.clone(),
        })
    }

    fn lower_type_expr(&mut self, sexpr: &SExpr) -> Result<TypeExpr, String> {
        match &sexpr.kind {
            SExprKind::Atom(AtomKind::Symbol(name)) => Ok(TypeExpr::Named(name.clone())),
            SExprKind::Map(entries) => {
                let mut fields = Vec::new();
                for (key, value) in entries {
                    let key_name = key
                        .as_keyword()
                        .or_else(|| key.as_symbol())
                        .ok_or("expected map type key")?
                        .to_string();
                    let val_type = self.lower_type_expr(value)?;
                    fields.push((key_name, val_type));
                }
                Ok(TypeExpr::Map(fields))
            }
            SExprKind::List(items) => {
                if let Some(head) = items.first().and_then(|s| s.as_symbol()) {
                    match head {
                        "list" => {
                            let inner = items
                                .get(1)
                                .map(|s| self.lower_type_expr(s))
                                .ok_or("expected list element type")??;
                            Ok(TypeExpr::List(Box::new(inner)))
                        }
                        "union" => {
                            let mut variants = Vec::new();
                            for item in &items[1..] {
                                variants.push(self.lower_variant(item)?);
                            }
                            Ok(TypeExpr::Union(variants))
                        }
                        "enum" => {
                            let mut names = Vec::new();
                            for item in &items[1..] {
                                if let Some(kw) = item.as_keyword() {
                                    names.push(kw.to_string());
                                }
                            }
                            Ok(TypeExpr::Enum(names))
                        }
                        _ => Ok(TypeExpr::Named(head.to_string())),
                    }
                } else {
                    Err("expected type expression".to_string())
                }
            }
            _ => Err(format!("unexpected type expression")),
        }
    }

    fn lower_effect_set(&mut self, sexpr: &SExpr) -> Result<EffectSetDef, String> {
        let items = sexpr.as_list().ok_or("expected effect-set to be a list")?;
        // (effect-set name [effects...])
        let name = items
            .get(1)
            .and_then(|s| s.as_symbol())
            .ok_or("expected effect-set name")?
            .to_string();

        let mut effects = Vec::new();
        if let Some(effect_items) = items.get(2).and_then(|s| s.as_vector()) {
            let mut j = 0;
            while j < effect_items.len() {
                if let Some(kw) = effect_items[j].as_keyword() {
                    let kind = match kw {
                        "reads" => EffectKind::Reads,
                        "writes" => EffectKind::Writes,
                        "sends" => EffectKind::Sends,
                        _ => {
                            j += 1;
                            continue;
                        }
                    };
                    j += 1;
                    if j < effect_items.len() {
                        if let Some(target) = effect_items[j].as_symbol() {
                            effects.push(Effect {
                                kind,
                                target: target.to_string(),
                            });
                        }
                    }
                }
                j += 1;
            }
        }

        Ok(EffectSetDef {
            name,
            effects,
            span: sexpr.span.clone(),
        })
    }

    fn lower_fn_def(&mut self, sexpr: &SExpr) -> Result<FnDef, String> {
        let items = sexpr.as_list().ok_or("expected fn to be a list")?;
        // (fn name :keyword value ... (param ...) (returns ...) body)
        let name = items
            .get(1)
            .and_then(|s| s.as_symbol())
            .ok_or("expected function name")?
            .to_string();

        let mut provenance = None;
        let mut effects = Vec::new();
        let mut total = false;
        let mut latency_budget = None;
        let mut called_by = Vec::new();
        let mut idempotency_key = None;
        let mut params = Vec::new();
        let mut returns = None;
        let mut body = None;
        let mut extra_meta = Vec::new();

        let mut i = 2;
        while i < items.len() {
            if let Some(kw) = items[i].as_keyword() {
                match kw {
                    "provenance" => {
                        i += 1;
                        provenance = Some(self.lower_provenance(&items[i])?);
                    }
                    "effects" => {
                        i += 1;
                        if let Some(effect_items) = items[i].as_vector() {
                            for item in effect_items {
                                if let Some(name) = item.as_symbol() {
                                    effects.push(name.to_string());
                                }
                            }
                        }
                    }
                    "total" => {
                        i += 1;
                        total = items[i].as_bool().unwrap_or(false);
                    }
                    "latency-budget" => {
                        i += 1;
                        if let SExprKind::Atom(AtomKind::DurationLit(val, unit)) = &items[i].kind {
                            latency_budget = Some(Duration {
                                value: *val,
                                unit: *unit,
                            });
                        }
                    }
                    "called-by" => {
                        i += 1;
                        if let Some(cb_items) = items[i].as_vector() {
                            for item in cb_items {
                                if let Some(name) = item.as_symbol() {
                                    called_by.push(name.to_string());
                                }
                            }
                        }
                    }
                    "idempotency-key" => {
                        i += 1;
                        idempotency_key = Some(self.lower_expr(&items[i])?);
                    }
                    other => {
                        i += 1;
                        if i < items.len() {
                            extra_meta.push((other.to_string(), self.sexpr_to_meta(&items[i])));
                        }
                    }
                }
                i += 1;
            } else if let Some(list) = items[i].as_list() {
                if let Some(head) = list.first().and_then(|s| s.as_symbol()) {
                    match head {
                        "param" => params.push(self.lower_param_def(&items[i])?),
                        "returns" => returns = Some(self.lower_returns_def(&items[i])?),
                        _ => {
                            // This is the body expression
                            body = Some(self.lower_expr(&items[i])?);
                        }
                    }
                }
                i += 1;
            } else {
                i += 1;
            }
        }

        let returns = returns.ok_or("expected (returns ...) in function")?;
        let body = body.ok_or("expected body expression in function")?;

        Ok(FnDef {
            name,
            provenance,
            effects,
            total,
            latency_budget,
            called_by,
            idempotency_key,
            params,
            returns,
            body,
            extra_meta,
            span: sexpr.span.clone(),
        })
    }

    fn lower_param_def(&mut self, sexpr: &SExpr) -> Result<ParamDef, String> {
        let items = sexpr.as_list().ok_or("expected param to be a list")?;
        // (param name TypeExpr :keyword value ...)
        let name = items
            .get(1)
            .and_then(|s| s.as_symbol())
            .ok_or("expected param name")?
            .to_string();
        let type_expr = items
            .get(2)
            .map(|s| self.lower_type_expr(s))
            .ok_or("expected param type")??;

        let mut source = None;
        let mut content_type = None;
        let mut validated_at = None;
        let mut extra_meta = Vec::new();

        let mut i = 3;
        while i < items.len() {
            if let Some(kw) = items[i].as_keyword() {
                match kw {
                    "source" => {
                        i += 1;
                        source = items
                            .get(i)
                            .and_then(|s| s.as_symbol().or_else(|| s.as_keyword()))
                            .map(|s| s.to_string());
                    }
                    "content-type" => {
                        i += 1;
                        content_type = items
                            .get(i)
                            .and_then(|s| s.as_keyword())
                            .map(|s| s.to_string());
                    }
                    "validated-at" => {
                        i += 1;
                        validated_at = items
                            .get(i)
                            .and_then(|s| s.as_symbol())
                            .map(|s| s.to_string());
                    }
                    other => {
                        i += 1;
                        if i < items.len() {
                            extra_meta.push((other.to_string(), self.sexpr_to_meta(&items[i])));
                        }
                    }
                }
                i += 1;
            } else {
                i += 1;
            }
        }

        Ok(ParamDef {
            name,
            type_expr,
            source,
            content_type,
            validated_at,
            extra_meta,
            span: sexpr.span.clone(),
        })
    }

    fn lower_returns_def(&mut self, sexpr: &SExpr) -> Result<ReturnsDef, String> {
        let items = sexpr.as_list().ok_or("expected returns to be a list")?;
        // (returns (union variant...))
        if items.len() < 2 {
            return Err("expected (returns (union ...))".to_string());
        }
        let union_items = items[1]
            .as_list()
            .ok_or("expected union in returns")?;
        if union_items.first().and_then(|s| s.as_symbol()) != Some("union") {
            return Err("expected (union ...) in returns".to_string());
        }

        let mut variants = Vec::new();
        for item in &union_items[1..] {
            variants.push(self.lower_variant(item)?);
        }

        Ok(ReturnsDef {
            variants,
            span: sexpr.span.clone(),
        })
    }

    fn lower_variant(&mut self, sexpr: &SExpr) -> Result<Variant, String> {
        let items = sexpr.as_list().ok_or("expected variant to be a list")?;
        let head = items
            .first()
            .and_then(|s| s.as_symbol())
            .ok_or("expected variant head")?;

        match head {
            "ok" => {
                let type_expr = items
                    .get(1)
                    .map(|s| self.lower_type_expr(s))
                    .ok_or("expected ok type")??;
                let mut http_status = None;
                let mut serialize = None;
                let mut extra_meta = Vec::new();

                let mut i = 2;
                while i < items.len() {
                    if let Some(kw) = items[i].as_keyword() {
                        match kw {
                            "http" => {
                                i += 1;
                                http_status = items.get(i).and_then(|s| s.as_int());
                            }
                            "serialize" => {
                                i += 1;
                                serialize = items
                                    .get(i)
                                    .and_then(|s| s.as_keyword())
                                    .map(|s| s.to_string());
                            }
                            other => {
                                i += 1;
                                if i < items.len() {
                                    extra_meta
                                        .push((other.to_string(), self.sexpr_to_meta(&items[i])));
                                }
                            }
                        }
                        i += 1;
                    } else {
                        i += 1;
                    }
                }

                Ok(Variant {
                    kind: VariantKind::Ok {
                        type_expr,
                        http_status,
                        serialize,
                        extra_meta,
                    },
                    span: sexpr.span.clone(),
                })
            }
            "err" => {
                // (err :tag payload :http N ...)
                let tag = items
                    .get(1)
                    .and_then(|s| s.as_keyword())
                    .ok_or("expected error tag keyword")?
                    .to_string();

                // Find the payload: it's the next non-keyword element after the tag
                let mut payload = TypeExpr::Named("Unit".to_string());
                let mut http_status = None;
                let mut extra_meta = Vec::new();

                let mut i = 2;
                // First non-keyword item is the payload
                if i < items.len() && items[i].as_keyword().is_none() {
                    payload = self.lower_type_expr(&items[i])?;
                    i += 1;
                }

                while i < items.len() {
                    if let Some(kw) = items[i].as_keyword() {
                        match kw {
                            "http" => {
                                i += 1;
                                http_status = items.get(i).and_then(|s| s.as_int());
                            }
                            other => {
                                i += 1;
                                if i < items.len() {
                                    extra_meta
                                        .push((other.to_string(), self.sexpr_to_meta(&items[i])));
                                }
                            }
                        }
                        i += 1;
                    } else {
                        i += 1;
                    }
                }

                Ok(Variant {
                    kind: VariantKind::Err {
                        tag,
                        payload,
                        http_status,
                        extra_meta,
                    },
                    span: sexpr.span.clone(),
                })
            }
            _ => Err(format!("expected 'ok' or 'err' variant, got '{}'", head)),
        }
    }

    fn lower_expr(&mut self, sexpr: &SExpr) -> Result<Expr, String> {
        match &sexpr.kind {
            SExprKind::Atom(AtomKind::Symbol(s)) => {
                if s == "_" {
                    Ok(Expr::Wildcard(sexpr.span.clone()))
                } else {
                    Ok(Expr::Ref(s.clone(), sexpr.span.clone()))
                }
            }
            SExprKind::Atom(AtomKind::Keyword(s)) => {
                Ok(Expr::Keyword(s.clone(), sexpr.span.clone()))
            }
            SExprKind::Atom(AtomKind::StringLit(s)) => {
                Ok(Expr::StringLit(s.clone(), sexpr.span.clone()))
            }
            SExprKind::Atom(AtomKind::IntLit(n)) => {
                Ok(Expr::IntLit(*n, sexpr.span.clone()))
            }
            SExprKind::Atom(AtomKind::BoolLit(b)) => {
                Ok(Expr::BoolLit(*b, sexpr.span.clone()))
            }
            SExprKind::Map(entries) => {
                let mut map = Vec::new();
                for (key, value) in entries {
                    let key_name = key
                        .as_symbol()
                        .or_else(|| key.as_keyword())
                        .ok_or("expected map key to be symbol or keyword")?
                        .to_string();
                    let val = self.lower_expr(value)?;
                    map.push((key_name, val));
                }
                Ok(Expr::MapLit(map, sexpr.span.clone()))
            }
            SExprKind::List(items) => {
                if items.is_empty() {
                    return Err("unexpected empty list in expression".to_string());
                }
                let head = items[0]
                    .as_symbol()
                    .ok_or("expected symbol at head of expression")?;

                match head {
                    "let" => self.lower_let(items, &sexpr.span),
                    "match" => self.lower_match(items, &sexpr.span),
                    "if" => self.lower_if(items, &sexpr.span),
                    "." => {
                        if items.len() != 3 {
                            return Err("expected (. expr field)".to_string());
                        }
                        let expr = self.lower_expr(&items[1])?;
                        let field = items[2]
                            .as_symbol()
                            .ok_or("expected field name")?
                            .to_string();
                        Ok(Expr::FieldAccess {
                            expr: Box::new(expr),
                            field,
                            span: sexpr.span.clone(),
                        })
                    }
                    "ok" => {
                        let inner = if items.len() > 1 {
                            self.lower_expr(&items[1])?
                        } else {
                            Expr::Ref("Unit".to_string(), sexpr.span.clone())
                        };
                        Ok(Expr::Ok(Box::new(inner), sexpr.span.clone()))
                    }
                    "err" => {
                        let tag = items
                            .get(1)
                            .and_then(|s| s.as_keyword())
                            .ok_or("expected error tag")?
                            .to_string();
                        let payload = if items.len() > 2 {
                            self.lower_expr(&items[2])?
                        } else {
                            Expr::Ref("Unit".to_string(), sexpr.span.clone())
                        };
                        Ok(Expr::Err {
                            tag,
                            payload: Box::new(payload),
                            span: sexpr.span.clone(),
                        })
                    }
                    _ => {
                        // Generic function call
                        let mut args = Vec::new();
                        for item in &items[1..] {
                            args.push(self.lower_expr(item)?);
                        }
                        Ok(Expr::Call {
                            name: head.to_string(),
                            args,
                            span: sexpr.span.clone(),
                        })
                    }
                }
            }
            _ => Err(format!("unexpected expression form")),
        }
    }

    fn lower_let(&mut self, items: &[SExpr], span: &Span) -> Result<Expr, String> {
        // (let [name1 expr1 name2 expr2 ...] body)
        if items.len() < 3 {
            return Err("let requires bindings and body".to_string());
        }
        let binding_items = items[1]
            .as_vector()
            .ok_or("expected let bindings to be a vector")?;

        let mut bindings = Vec::new();
        let mut j = 0;
        while j + 1 < binding_items.len() {
            let name = binding_items[j]
                .as_symbol()
                .ok_or("expected binding name")?
                .to_string();
            let value = self.lower_expr(&binding_items[j + 1])?;
            bindings.push((name, value));
            j += 2;
        }

        let body = self.lower_expr(&items[2])?;

        Ok(Expr::Let {
            bindings,
            body: Box::new(body),
            span: span.clone(),
        })
    }

    fn lower_match(&mut self, items: &[SExpr], span: &Span) -> Result<Expr, String> {
        // (match expr pattern1 body1 pattern2 body2 ...)
        if items.len() < 4 {
            return Err("match requires expression and at least one arm".to_string());
        }
        let expr = self.lower_expr(&items[1])?;

        let mut arms = Vec::new();
        let mut j = 2;
        while j + 1 < items.len() {
            let pattern = self.lower_pattern(&items[j])?;
            let body = self.lower_expr(&items[j + 1])?;
            let arm_span = Span::new(items[j].span.start, items[j + 1].span.end);
            arms.push(MatchArm {
                pattern,
                body,
                span: arm_span,
            });
            j += 2;
        }

        Ok(Expr::Match {
            expr: Box::new(expr),
            arms,
            span: span.clone(),
        })
    }

    fn lower_if(&mut self, items: &[SExpr], span: &Span) -> Result<Expr, String> {
        // (if cond then else)
        if items.len() != 4 {
            return Err("if requires condition, then, and else branches".to_string());
        }
        let cond = self.lower_expr(&items[1])?;
        let then_branch = self.lower_expr(&items[2])?;
        let else_branch = self.lower_expr(&items[3])?;

        Ok(Expr::If {
            cond: Box::new(cond),
            then_branch: Box::new(then_branch),
            else_branch: Box::new(else_branch),
            span: span.clone(),
        })
    }

    fn lower_pattern(&mut self, sexpr: &SExpr) -> Result<Pattern, String> {
        match &sexpr.kind {
            SExprKind::Atom(AtomKind::Symbol(s)) => {
                if s == "_" {
                    Ok(Pattern::Wildcard(sexpr.span.clone()))
                } else {
                    Ok(Pattern::Var(s.clone(), sexpr.span.clone()))
                }
            }
            SExprKind::Atom(AtomKind::Keyword(s)) => {
                Ok(Pattern::Keyword(s.clone(), sexpr.span.clone()))
            }
            SExprKind::List(items) => {
                if items.is_empty() {
                    return Err("unexpected empty pattern".to_string());
                }
                let name = items[0]
                    .as_symbol()
                    .ok_or("expected constructor name in pattern")?
                    .to_string();
                let mut args = Vec::new();
                for item in &items[1..] {
                    args.push(self.lower_pattern(item)?);
                }
                Ok(Pattern::Constructor {
                    name,
                    args,
                    span: sexpr.span.clone(),
                })
            }
            _ => Err("unexpected pattern form".to_string()),
        }
    }

    fn sexpr_to_meta(&self, sexpr: &SExpr) -> MetaValue {
        match &sexpr.kind {
            SExprKind::Atom(AtomKind::StringLit(s)) => MetaValue::String(s.clone()),
            SExprKind::Atom(AtomKind::IntLit(n)) => MetaValue::Int(*n),
            SExprKind::Atom(AtomKind::BoolLit(b)) => MetaValue::Bool(*b),
            SExprKind::Atom(AtomKind::Symbol(s)) => MetaValue::Symbol(s.clone()),
            SExprKind::Atom(AtomKind::Keyword(s)) => MetaValue::Keyword(s.clone()),
            SExprKind::Atom(AtomKind::DurationLit(v, u)) => MetaValue::Duration(*v, *u),
            SExprKind::Atom(AtomKind::RegexLit(s)) => MetaValue::String(s.clone()),
            SExprKind::Vector(items) => {
                MetaValue::List(items.iter().map(|i| self.sexpr_to_meta(i)).collect())
            }
            SExprKind::Map(entries) => {
                let mapped: Vec<(String, MetaValue)> = entries
                    .iter()
                    .map(|(k, v)| {
                        let key = k
                            .as_symbol()
                            .or_else(|| k.as_keyword())
                            .unwrap_or("?")
                            .to_string();
                        (key, self.sexpr_to_meta(v))
                    })
                    .collect();
                MetaValue::Map(mapped)
            }
            SExprKind::List(items) => {
                if let Ok(expr) = Lowerer::new().lower_expr(sexpr) {
                    MetaValue::Expr(expr)
                } else {
                    MetaValue::List(items.iter().map(|i| self.sexpr_to_meta(i)).collect())
                }
            }
        }
    }
}

/// Format an SExpr back to a string (for invariants, etc.)
fn format_sexpr(sexpr: &SExpr) -> String {
    match &sexpr.kind {
        SExprKind::Atom(AtomKind::Symbol(s)) => s.clone(),
        SExprKind::Atom(AtomKind::Keyword(s)) => format!(":{}", s),
        SExprKind::Atom(AtomKind::StringLit(s)) => format!("\"{}\"", s),
        SExprKind::Atom(AtomKind::IntLit(n)) => n.to_string(),
        SExprKind::Atom(AtomKind::BoolLit(b)) => b.to_string(),
        SExprKind::Atom(AtomKind::DurationLit(v, u)) => format!("{}{}", v, u),
        SExprKind::Atom(AtomKind::RegexLit(r)) => format!("#/{}/", r),
        SExprKind::List(items) => {
            let inner: Vec<String> = items.iter().map(format_sexpr).collect();
            format!("({})", inner.join(" "))
        }
        SExprKind::Vector(items) => {
            let inner: Vec<String> = items.iter().map(format_sexpr).collect();
            format!("[{}]", inner.join(" "))
        }
        SExprKind::Map(entries) => {
            let inner: Vec<String> = entries
                .iter()
                .map(|(k, v)| format!("{}: {}", format_sexpr(k), format_sexpr(v)))
                .collect();
            format!("{{{}}}", inner.join(", "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn parse_and_lower(input: &str) -> Module {
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let sexprs = parser.parse_program().unwrap();
        let mut lowerer = Lowerer::new();
        lowerer.lower_module(&sexprs[0]).unwrap()
    }

    #[test]
    fn test_lower_minimal_module() {
        let module = parse_and_lower("(module test-mod :version 1)");
        assert_eq!(module.name, "test-mod");
        assert_eq!(module.version, Some(1));
    }

    #[test]
    fn test_lower_type_def() {
        let module = parse_and_lower(
            "(module test :version 1 (type User (field id UUID :immutable :generated) (field name String :min-len 1)))"
        );
        assert_eq!(module.types.len(), 1);
        let t = &module.types[0];
        assert_eq!(t.name, "User");
        assert_eq!(t.fields.len(), 2);
        assert_eq!(t.fields[0].name, "id");
        assert!(t.fields[0].immutable);
        assert!(t.fields[0].generated);
        assert_eq!(t.fields[1].name, "name");
        assert_eq!(t.fields[1].min_len, Some(1));
    }

    #[test]
    fn test_lower_effect_set() {
        let module =
            parse_and_lower("(module test :version 1 (effect-set db-read [:reads user-store]))");
        assert_eq!(module.effect_sets.len(), 1);
        assert_eq!(module.effect_sets[0].name, "db-read");
        assert_eq!(module.effect_sets[0].effects.len(), 1);
        assert_eq!(module.effect_sets[0].effects[0].kind, EffectKind::Reads);
        assert_eq!(module.effect_sets[0].effects[0].target, "user-store");
    }

    #[test]
    fn test_lower_simple_fn() {
        let input = r#"(module test :version 1
            (fn get-thing
                :effects [db-read]
                :total true
                :latency-budget 50ms
                (param id UUID :source http-path-param)
                (returns (union
                    (ok Thing :http 200)
                    (err :not-found {:id id} :http 404)))
                (ok id)))"#;
        let module = parse_and_lower(input);
        assert_eq!(module.functions.len(), 1);
        let f = &module.functions[0];
        assert_eq!(f.name, "get-thing");
        assert_eq!(f.effects, vec!["db-read"]);
        assert!(f.total);
        assert_eq!(f.params.len(), 1);
        assert_eq!(f.params[0].name, "id");
        assert_eq!(f.returns.variants.len(), 2);
    }

    #[test]
    fn test_lower_full_example() {
        let source = std::fs::read_to_string(
            concat!(env!("CARGO_MANIFEST_DIR"), "/examples/user-service.pct"),
        )
        .unwrap();
        let mut lexer = Lexer::new(&source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let sexprs = parser.parse_program().unwrap();
        let mut lowerer = Lowerer::new();
        let module = lowerer.lower_module(&sexprs[0]).unwrap();

        assert_eq!(module.name, "user-service");
        assert_eq!(module.version, Some(7));
        assert_eq!(module.parent_version, Some(6));
        assert!(module.provenance.is_some());
        assert!(module.delta.is_some());

        // Types
        assert_eq!(module.types.len(), 1);
        assert_eq!(module.types[0].name, "User");
        assert_eq!(module.types[0].fields.len(), 3);
        assert_eq!(module.types[0].invariants.len(), 2);

        // Effect sets
        assert_eq!(module.effect_sets.len(), 3);

        // Functions
        assert_eq!(module.functions.len(), 2);
        assert_eq!(module.functions[0].name, "get-user-by-id");
        assert_eq!(module.functions[1].name, "create-user");

        // Check first function details
        let get_fn = &module.functions[0];
        assert!(get_fn.total);
        assert_eq!(get_fn.effects, vec!["db-read", "http-respond"]);
        assert_eq!(get_fn.params.len(), 1);
        assert_eq!(get_fn.returns.variants.len(), 3);
    }
}
