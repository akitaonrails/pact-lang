use crate::ast::*;

/// HTTP method for a route
#[derive(Debug, Clone, PartialEq)]
pub enum HttpMethod {
    Get,
    Post,
}

/// The kind of route being generated
#[derive(Debug, Clone, PartialEq)]
pub enum RouteKind {
    List,
    NewForm,
    Create,
    Show,
    Delete,
}

/// Information about a store type extracted from effect sets
#[derive(Debug, Clone)]
pub struct StoreInfo {
    pub type_name: String,    // "User"
    pub plural: String,       // "users"
    pub singular: String,     // "user"
    pub needs_mut: bool,      // any function writes to it
}

/// Information about a form field for HTML generation
#[derive(Debug, Clone)]
pub struct FormField {
    pub name: String,         // "name", "email"
    pub label: String,        // "Name", "Email"
    pub input_type: String,   // "text", "email"
    pub min_len: Option<i64>,
    pub max_len: Option<i64>,
    pub required: bool,
}

/// Metadata about a function-backed route
#[derive(Debug, Clone)]
pub struct FnRoute {
    pub fn_name: String,         // "get_user_by_id"
    pub result_enum: String,     // "GetUserByIdResult"
    pub input_struct: Option<String>, // "CreateUserInput"
    pub variants: Vec<RouteVariant>,
}

/// A variant in a function's return type, with HTTP metadata
#[derive(Debug, Clone)]
pub struct RouteVariant {
    pub is_ok: bool,
    pub tag: Option<String>,     // None for Ok, Some("not-found") for Err
    pub variant_name: String,    // "Ok", "NotFound"
    pub http_status: u16,
    pub payload_kind: PayloadKind,
}

#[derive(Debug, Clone)]
pub enum PayloadKind {
    Type(String),         // "User"
    Map(Vec<(String, String)>), // [("id", "String")]
    List(String),         // "Vec<ValidationError>"
    Unit,
}

/// A single route in the generated web app
#[derive(Debug, Clone)]
pub struct Route {
    pub kind: RouteKind,
    pub method: HttpMethod,
    pub path: String,              // "/users/{id}"
    pub api_path: Option<String>,  // "/api/users/{id}"
    pub handler_name: String,      // "show_user"
    pub api_handler_name: Option<String>, // "api_get_user"
    pub function: Option<FnRoute>,
    pub store_type: String,        // "User"
    pub form_fields: Vec<FormField>,
}

/// The complete route table extracted from a module
#[derive(Debug, Clone)]
pub struct RouteTable {
    pub module_name: String,
    pub store_types: Vec<StoreInfo>,
    pub routes: Vec<Route>,
}

/// Analyze an AST Module and produce a RouteTable
pub fn analyze(module: &Module) -> RouteTable {
    let module_name = module.name.replace('-', "_");
    let store_types = collect_store_types(module);
    let mut routes = Vec::new();

    // For each store type, generate implicit routes (list, new-form, delete)
    for store in &store_types {
        // GET /{plural} → list
        routes.push(Route {
            kind: RouteKind::List,
            method: HttpMethod::Get,
            path: format!("/{}", store.plural),
            api_path: Some(format!("/api/{}", store.plural)),
            handler_name: format!("list_{}", store.plural),
            api_handler_name: Some(format!("api_list_{}", store.plural)),
            function: None,
            store_type: store.type_name.clone(),
            form_fields: vec![],
        });

        // GET /{plural}/new → new form
        routes.push(Route {
            kind: RouteKind::NewForm,
            method: HttpMethod::Get,
            path: format!("/{}/new", store.plural),
            api_path: None,
            handler_name: format!("new_{}_form", store.singular),
            api_handler_name: None,
            function: None,
            store_type: store.type_name.clone(),
            form_fields: vec![],
        });

        // POST /{plural}/{id}/delete → delete
        routes.push(Route {
            kind: RouteKind::Delete,
            method: HttpMethod::Post,
            path: format!("/{}/{{id}}/delete", store.plural),
            api_path: None,
            handler_name: format!("delete_{}", store.singular),
            api_handler_name: None,
            function: None,
            store_type: store.type_name.clone(),
            form_fields: vec![],
        });
    }

    // For each function, generate routes based on param sources and effects
    for func in &module.functions {
        let fn_routes = analyze_function(func, module, &store_types);
        routes.extend(fn_routes);
    }

    // Populate form fields for NewForm routes from type definitions
    for route in &mut routes {
        if route.kind == RouteKind::NewForm {
            if let Some(typedef) = module.types.iter().find(|t| t.name == route.store_type) {
                route.form_fields = extract_form_fields(typedef);
            }
        }
    }

    RouteTable {
        module_name,
        store_types,
        routes,
    }
}

/// Analyze a single function and generate routes for it
fn analyze_function(func: &FnDef, module: &Module, store_types: &[StoreInfo]) -> Vec<Route> {
    let mut routes = Vec::new();

    let has_writes = func_has_writes(func, module);
    let fn_name = to_snake(&func.name);
    let result_enum = format!("{}Result", to_pascal(&func.name));

    // Determine input struct name if function has map params
    let input_struct = func.params.iter().find_map(|p| {
        if let TypeExpr::Map(_) = &p.type_expr {
            Some(format!("{}Input", to_pascal(&func.name)))
        } else {
            None
        }
    });

    // Extract variant info
    let variants: Vec<RouteVariant> = func.returns.variants.iter().map(|v| {
        match &v.kind {
            VariantKind::Ok { type_expr, http_status, .. } => RouteVariant {
                is_ok: true,
                tag: None,
                variant_name: "Ok".to_string(),
                http_status: http_status.unwrap_or(200) as u16,
                payload_kind: type_expr_to_payload(type_expr),
            },
            VariantKind::Err { tag, payload, http_status, .. } => RouteVariant {
                is_ok: false,
                tag: Some(tag.clone()),
                variant_name: to_pascal(tag),
                http_status: http_status.unwrap_or(500) as u16,
                payload_kind: type_expr_to_payload(payload),
            },
        }
    }).collect();

    let fn_route = FnRoute {
        fn_name: fn_name.clone(),
        result_enum,
        input_struct: input_struct.clone(),
        variants,
    };

    // Find the store type for this function
    let store_type = find_fn_store_type(func, module, store_types);

    if has_writes && input_struct.is_some() {
        // POST /{plural} → create (both HTML and API)
        if let Some(store) = &store_type {
            // Extract form fields from the map param
            let form_fields = func.params.iter().find_map(|p| {
                if let TypeExpr::Map(fields) = &p.type_expr {
                    Some(extract_form_fields_from_map(fields, module))
                } else {
                    None
                }
            }).unwrap_or_default();

            routes.push(Route {
                kind: RouteKind::Create,
                method: HttpMethod::Post,
                path: format!("/{}", store.plural),
                api_path: Some(format!("/api/{}", store.plural)),
                handler_name: format!("create_{}_handler", store.singular),
                api_handler_name: Some(format!("api_create_{}", store.singular)),
                function: Some(fn_route),
                store_type: store.type_name.clone(),
                form_fields,
            });
        }
    } else {
        // Read-only function with UUID path param → GET /{plural}/{id} (show)
        let has_uuid_path_param = func.params.iter().any(|p| {
            matches!(&p.type_expr, TypeExpr::Named(n) if n == "UUID")
                && p.source.as_deref() == Some("http-path-param")
        });

        if has_uuid_path_param {
            if let Some(store) = &store_type {
                routes.push(Route {
                    kind: RouteKind::Show,
                    method: HttpMethod::Get,
                    path: format!("/{}/{{id}}", store.plural),
                    api_path: Some(format!("/api/{}/{{id}}", store.plural)),
                    handler_name: format!("show_{}", store.singular),
                    api_handler_name: Some(format!("api_get_{}", store.singular)),
                    function: Some(fn_route),
                    store_type: store.type_name.clone(),
                    form_fields: vec![],
                });
            }
        }
    }

    routes
}

/// Collect all store types from the module's effect sets
fn collect_store_types(module: &Module) -> Vec<StoreInfo> {
    let mut stores: Vec<StoreInfo> = Vec::new();

    for effect_set in &module.effect_sets {
        for effect in &effect_set.effects {
            if matches!(effect.kind, EffectKind::Sends) {
                continue;
            }
            let type_name = store_target_to_type(&effect.target);
            let needs_mut = matches!(effect.kind, EffectKind::Writes);

            if let Some(existing) = stores.iter_mut().find(|s| s.type_name == type_name) {
                if needs_mut {
                    existing.needs_mut = true;
                }
            } else {
                let singular = type_name.to_lowercase();
                let plural = pluralize(&singular);
                stores.push(StoreInfo {
                    type_name,
                    plural,
                    singular,
                    needs_mut,
                });
            }
        }
    }

    stores
}

/// Check if a function has any write effects
fn func_has_writes(func: &FnDef, module: &Module) -> bool {
    for effect_name in &func.effects {
        if let Some(es) = module.effect_sets.iter().find(|es| &es.name == effect_name) {
            if es.effects.iter().any(|e| matches!(e.kind, EffectKind::Writes)) {
                return true;
            }
        }
    }
    false
}

/// Find the store type associated with a function
fn find_fn_store_type<'a>(func: &FnDef, module: &Module, store_types: &'a [StoreInfo]) -> Option<&'a StoreInfo> {
    for effect_name in &func.effects {
        if let Some(es) = module.effect_sets.iter().find(|es| &es.name == effect_name) {
            for effect in &es.effects {
                if matches!(effect.kind, EffectKind::Sends) {
                    continue;
                }
                let type_name = store_target_to_type(&effect.target);
                if let Some(store) = store_types.iter().find(|s| s.type_name == type_name) {
                    return Some(store);
                }
            }
        }
    }
    None
}

/// Extract form fields from a type definition (for new-form routes)
fn extract_form_fields(typedef: &TypeDef) -> Vec<FormField> {
    typedef.fields.iter()
        .filter(|f| !f.generated && !f.immutable)
        .map(|f| {
            let input_type = if f.format.as_deref() == Some("email") {
                "email".to_string()
            } else {
                match &f.type_expr {
                    TypeExpr::Named(n) if n == "Int" => "number".to_string(),
                    TypeExpr::Named(n) if n == "Bool" => "checkbox".to_string(),
                    _ => "text".to_string(),
                }
            };

            FormField {
                name: to_snake(&f.name),
                label: to_title(&f.name),
                input_type,
                min_len: f.min_len,
                max_len: f.max_len,
                required: true,
            }
        })
        .collect()
}

/// Extract form fields from a Map type expression (for create routes)
fn extract_form_fields_from_map(fields: &[(String, TypeExpr)], module: &Module) -> Vec<FormField> {
    fields.iter().map(|(name, type_expr)| {
        // Try to find field constraints from type definitions
        let (min_len, max_len, format) = find_field_constraints(name, module);

        let input_type = if format.as_deref() == Some("email") {
            "email".to_string()
        } else {
            match type_expr {
                TypeExpr::Named(n) if n == "Int" => "number".to_string(),
                TypeExpr::Named(n) if n == "Bool" => "checkbox".to_string(),
                _ => "text".to_string(),
            }
        };

        FormField {
            name: to_snake(name),
            label: to_title(name),
            input_type,
            min_len,
            max_len,
            required: true,
        }
    }).collect()
}

/// Look up field constraints from type definitions in the module
fn find_field_constraints(field_name: &str, module: &Module) -> (Option<i64>, Option<i64>, Option<String>) {
    for typedef in &module.types {
        if let Some(field) = typedef.fields.iter().find(|f| f.name == *field_name) {
            return (field.min_len, field.max_len, field.format.clone());
        }
    }
    (None, None, None)
}

/// Convert TypeExpr to PayloadKind
fn type_expr_to_payload(type_expr: &TypeExpr) -> PayloadKind {
    match type_expr {
        TypeExpr::Named(n) if n == "Unit" => PayloadKind::Unit,
        TypeExpr::Named(n) => PayloadKind::Type(n.clone()),
        TypeExpr::Map(fields) => {
            PayloadKind::Map(fields.iter().map(|(k, v)| {
                (to_snake(k), type_expr_to_rust_simple(v))
            }).collect())
        }
        TypeExpr::List(inner) => {
            PayloadKind::List(format!("Vec<{}>", type_expr_to_rust_simple(inner)))
        }
        _ => PayloadKind::Unit,
    }
}

fn type_expr_to_rust_simple(type_expr: &TypeExpr) -> String {
    match type_expr {
        TypeExpr::Named(name) => match name.as_str() {
            "UUID" => "Uuid".to_string(),
            "String" => "String".to_string(),
            "Int" => "i64".to_string(),
            "Bool" => "bool".to_string(),
            other => other.to_string(),
        },
        TypeExpr::List(inner) => format!("Vec<{}>", type_expr_to_rust_simple(inner)),
        _ => "String".to_string(),
    }
}

/// Convert store target name to type name
fn store_target_to_type(target: &str) -> String {
    let name = target
        .strip_suffix("-store")
        .or_else(|| target.strip_suffix("_store"))
        .unwrap_or(target);
    to_pascal(name)
}

/// Simple pluralization (just adds "s")
fn pluralize(s: &str) -> String {
    format!("{}s", s)
}

fn to_snake(name: &str) -> String {
    name.replace('-', "_")
        .replace('/', "_")
        .replace('?', "")
        .replace('!', "")
}

fn to_pascal(name: &str) -> String {
    name.split(|c| c == '-' || c == '_' || c == '/')
        .map(|part| {
            let part = part.replace('?', "").replace('!', "");
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => {
                    let upper = c.to_uppercase().to_string();
                    upper + &chars.as_str().to_lowercase()
                }
            }
        })
        .collect()
}

fn to_title(name: &str) -> String {
    name.split(|c: char| c == '-' || c == '_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => {
                    let upper = c.to_uppercase().to_string();
                    upper + chars.as_str()
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::lower::Lowerer;
    use crate::parser::Parser;

    fn parse_module(input: &str) -> Module {
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let sexprs = parser.parse_program().unwrap();
        let mut lowerer = Lowerer::new();
        lowerer.lower_module(&sexprs[0]).unwrap()
    }

    #[test]
    fn test_collect_store_types() {
        let module = parse_module(
            "(module test :version 1
                (type User (field id UUID :immutable :generated) (field name String))
                (effect-set db-read [:reads user-store])
                (effect-set db-write [:writes user-store :reads user-store]))"
        );
        let stores = collect_store_types(&module);
        assert_eq!(stores.len(), 1);
        assert_eq!(stores[0].type_name, "User");
        assert_eq!(stores[0].plural, "users");
        assert_eq!(stores[0].singular, "user");
        assert!(stores[0].needs_mut);
    }

    #[test]
    fn test_analyze_produces_implicit_routes() {
        let module = parse_module(
            "(module test :version 1
                (type User (field id UUID :immutable :generated) (field name String))
                (effect-set db-read [:reads user-store]))"
        );
        let table = analyze(&module);
        assert_eq!(table.module_name, "test");

        let list = table.routes.iter().find(|r| r.kind == RouteKind::List);
        assert!(list.is_some());
        assert_eq!(list.unwrap().path, "/users");

        let new_form = table.routes.iter().find(|r| r.kind == RouteKind::NewForm);
        assert!(new_form.is_some());
        assert_eq!(new_form.unwrap().path, "/users/new");

        let delete = table.routes.iter().find(|r| r.kind == RouteKind::Delete);
        assert!(delete.is_some());
        assert_eq!(delete.unwrap().path, "/users/{id}/delete");
    }

    #[test]
    fn test_analyze_show_route() {
        let source = std::fs::read_to_string(
            concat!(env!("CARGO_MANIFEST_DIR"), "/examples/user-service.pct"),
        ).unwrap();
        let module = parse_module(&source);
        let table = analyze(&module);

        let show = table.routes.iter().find(|r| r.kind == RouteKind::Show);
        assert!(show.is_some(), "Should have a Show route");
        let show = show.unwrap();
        assert_eq!(show.path, "/users/{id}");
        assert_eq!(show.method, HttpMethod::Get);
        assert!(show.function.is_some());
        let fn_route = show.function.as_ref().unwrap();
        assert_eq!(fn_route.fn_name, "get_user_by_id");
        assert_eq!(fn_route.variants.len(), 3);
    }

    #[test]
    fn test_analyze_create_route() {
        let source = std::fs::read_to_string(
            concat!(env!("CARGO_MANIFEST_DIR"), "/examples/user-service.pct"),
        ).unwrap();
        let module = parse_module(&source);
        let table = analyze(&module);

        let create = table.routes.iter().find(|r| r.kind == RouteKind::Create);
        assert!(create.is_some(), "Should have a Create route");
        let create = create.unwrap();
        assert_eq!(create.path, "/users");
        assert_eq!(create.method, HttpMethod::Post);
        assert!(create.function.is_some());
        let fn_route = create.function.as_ref().unwrap();
        assert_eq!(fn_route.fn_name, "create_user");
        assert!(fn_route.input_struct.is_some());
        assert_eq!(create.form_fields.len(), 2);
    }

    #[test]
    fn test_form_fields_from_type() {
        let module = parse_module(
            "(module test :version 1
                (type User
                    (field id UUID :immutable :generated)
                    (field name String :min-len 1 :max-len 200)
                    (field email String :format :email)))"
        );
        let fields = extract_form_fields(&module.types[0]);
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name, "name");
        assert_eq!(fields[0].input_type, "text");
        assert_eq!(fields[0].min_len, Some(1));
        assert_eq!(fields[0].max_len, Some(200));
        assert_eq!(fields[1].name, "email");
        assert_eq!(fields[1].input_type, "email");
    }

    #[test]
    fn test_route_variants() {
        let source = std::fs::read_to_string(
            concat!(env!("CARGO_MANIFEST_DIR"), "/examples/user-service.pct"),
        ).unwrap();
        let module = parse_module(&source);
        let table = analyze(&module);

        let show = table.routes.iter().find(|r| r.kind == RouteKind::Show).unwrap();
        let fn_route = show.function.as_ref().unwrap();

        let ok = &fn_route.variants[0];
        assert!(ok.is_ok);
        assert_eq!(ok.http_status, 200);

        let not_found = &fn_route.variants[1];
        assert!(!not_found.is_ok);
        assert_eq!(not_found.tag.as_deref(), Some("not-found"));
        assert_eq!(not_found.http_status, 404);
    }

    #[test]
    fn test_pluralize() {
        assert_eq!(pluralize("user"), "users");
        assert_eq!(pluralize("item"), "items");
    }

    #[test]
    fn test_to_title() {
        assert_eq!(to_title("name"), "Name");
        assert_eq!(to_title("email-address"), "Email Address");
        assert_eq!(to_title("first_name"), "First Name");
    }
}
