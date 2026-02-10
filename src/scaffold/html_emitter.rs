use super::route_analysis::RouteTable;

/// Generate the html.rs helper file (static Tailwind utilities).
/// This is mostly identical to pact-web/src/html.rs but parameterized with the app title.
pub fn emit(table: &RouteTable) -> String {
    let app_title = to_title_case(&table.module_name);
    let nav_links = build_nav_links(table);

    let mut out = String::new();

    // html_page function — uses push_str to avoid nested format! escaping issues
    out.push_str("/// Wrap body content in a full HTML page with Tailwind CDN\n");
    out.push_str("pub fn html_page(title: &str, body: &str) -> String {\n");
    out.push_str("    format!(\n");
    out.push_str("        r##\"<!DOCTYPE html>\n");
    out.push_str("<html lang=\"en\">\n");
    out.push_str("<head>\n");
    out.push_str("    <meta charset=\"UTF-8\">\n");
    out.push_str("    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
    out.push_str(&format!("    <title>{{title}} - {}</title>\n", app_title));
    out.push_str("    <script src=\"https://cdn.tailwindcss.com\"></script>\n");
    out.push_str("</head>\n");
    out.push_str("<body class=\"bg-gray-50 min-h-screen\">\n");
    out.push_str("    {nav}\n");
    out.push_str("    <main class=\"max-w-4xl mx-auto py-8 px-4\">\n");
    out.push_str("        {body}\n");
    out.push_str("    </main>\n");
    out.push_str("</body>\n");
    out.push_str("</html>\"##,\n");
    out.push_str("        title = title,\n");
    out.push_str(&format!("        nav = html_nav(\"{}\", &[{}]),\n", app_title, nav_links));
    out.push_str("        body = body,\n");
    out.push_str("    )\n");
    out.push_str("}\n");

    out.push('\n');

    // html_nav function
    out.push_str(concat!(
        "/// Top navigation bar\n",
        "pub fn html_nav(title: &str, links: &[(&str, &str)]) -> String {\n",
        "    let link_items: String = links\n",
        "        .iter()\n",
        "        .map(|(href, label)| {\n",
        "            format!(\n",
        "                r#\"<a href=\"{href}\" class=\"text-gray-300 hover:text-white px-3 py-2 text-sm font-medium\">{label}</a>\"#,\n",
        "                href = href,\n",
        "                label = label,\n",
        "            )\n",
        "        })\n",
        "        .collect();\n",
        "\n",
        "    format!(\n",
        "        r#\"<nav class=\"bg-gray-800\">\n",
        "    <div class=\"max-w-4xl mx-auto px-4 py-3 flex items-center justify-between\">\n",
        "        <span class=\"text-white font-bold text-lg\">{title}</span>\n",
        "        <div class=\"flex space-x-4\">{links}</div>\n",
        "    </div>\n",
        "</nav>\"#,\n",
        "        title = title,\n",
        "        links = link_items,\n",
        "    )\n",
        "}\n",
    ));

    out.push('\n');

    // html_table function
    out.push_str(concat!(
        "/// Render a Tailwind-styled table\n",
        "pub fn html_table(headers: &[&str], rows: &[Vec<String>]) -> String {\n",
        "    let header_cells: String = headers\n",
        "        .iter()\n",
        "        .map(|h| format!(r#\"<th class=\"px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider\">{h}</th>\"#, h = h))\n",
        "        .collect();\n",
        "\n",
        "    let body_rows: String = rows\n",
        "        .iter()\n",
        "        .map(|row| {\n",
        "            let cells: String = row\n",
        "                .iter()\n",
        "                .map(|cell| {\n",
        "                    format!(\n",
        "                        r#\"<td class=\"px-6 py-4 whitespace-nowrap text-sm text-gray-900\">{cell}</td>\"#,\n",
        "                        cell = cell,\n",
        "                    )\n",
        "                })\n",
        "                .collect();\n",
        "            format!(\"<tr class=\\\"hover:bg-gray-50\\\">{cells}</tr>\", cells = cells)\n",
        "        })\n",
        "        .collect();\n",
        "\n",
        "    format!(\n",
        "        r#\"<div class=\"overflow-hidden shadow ring-1 ring-black ring-opacity-5 rounded-lg\">\n",
        "    <table class=\"min-w-full divide-y divide-gray-300\">\n",
        "        <thead class=\"bg-gray-50\">\n",
        "            <tr>{headers}</tr>\n",
        "        </thead>\n",
        "        <tbody class=\"divide-y divide-gray-200 bg-white\">{body}</tbody>\n",
        "    </table>\n",
        "</div>\"#,\n",
        "        headers = header_cells,\n",
        "        body = body_rows,\n",
        "    )\n",
        "}\n",
    ));

    out.push('\n');

    // html_form function
    out.push_str(concat!(
        "/// Render a Tailwind-styled form\n",
        "pub fn html_form(action: &str, fields: &[(&str, &str, &str)]) -> String {\n",
        "    let field_html: String = fields\n",
        "        .iter()\n",
        "        .map(|(name, label, input_type)| {\n",
        "            format!(\n",
        "                r#\"<div class=\"mb-4\">\n",
        "    <label for=\"{name}\" class=\"block text-sm font-medium text-gray-700 mb-1\">{label}</label>\n",
        "    <input type=\"{input_type}\" name=\"{name}\" id=\"{name}\"\n",
        "        class=\"block w-full rounded-md border-gray-300 shadow-sm focus:border-indigo-500 focus:ring-indigo-500 sm:text-sm px-3 py-2 border\"\n",
        "        required>\n",
        "</div>\"#,\n",
        "                name = name,\n",
        "                label = label,\n",
        "                input_type = input_type,\n",
        "            )\n",
        "        })\n",
        "        .collect();\n",
        "\n",
        "    format!(\n",
        "        r#\"<form method=\"POST\" action=\"{action}\" class=\"bg-white shadow rounded-lg p-6 max-w-md\">\n",
        "    {fields}\n",
        "    <button type=\"submit\"\n",
        "        class=\"w-full bg-indigo-600 text-white py-2 px-4 rounded-md hover:bg-indigo-700 focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:ring-offset-2 font-medium\">\n",
        "        Submit\n",
        "    </button>\n",
        "</form>\"#,\n",
        "        action = action,\n",
        "        fields = field_html,\n",
        "    )\n",
        "}\n",
    ));

    out.push('\n');

    // html_alert function
    out.push_str(concat!(
        "/// Success/error alert\n",
        "pub fn html_alert(kind: &str, message: &str) -> String {\n",
        "    let (bg, border, text) = match kind {\n",
        "        \"success\" => (\"bg-green-50\", \"border-green-400\", \"text-green-700\"),\n",
        "        \"error\" => (\"bg-red-50\", \"border-red-400\", \"text-red-700\"),\n",
        "        \"warning\" => (\"bg-yellow-50\", \"border-yellow-400\", \"text-yellow-700\"),\n",
        "        _ => (\"bg-blue-50\", \"border-blue-400\", \"text-blue-700\"),\n",
        "    };\n",
        "    format!(\n",
        "        r#\"<div class=\"{bg} border-l-4 {border} p-4 mb-4\">\n",
        "    <p class=\"{text}\">{message}</p>\n",
        "</div>\"#,\n",
        "        bg = bg,\n",
        "        border = border,\n",
        "        text = text,\n",
        "        message = message,\n",
        "    )\n",
        "}\n",
    ));

    out
}

fn build_nav_links(table: &RouteTable) -> String {
    let mut links = Vec::new();
    for store in &table.store_types {
        let title = to_title_case(&store.plural);
        links.push(format!("(\"/\", \"{}\")", title));
        links.push(format!("(\"/{}/new\", \"New {}\")", store.plural, to_title_case(&store.singular)));
    }
    links.join(", ")
}

fn to_title_case(name: &str) -> String {
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
    use crate::scaffold::route_analysis::{StoreInfo, RouteTable};

    fn test_table() -> RouteTable {
        RouteTable {
            module_name: "user_service".to_string(),
            store_types: vec![StoreInfo {
                type_name: "User".to_string(),
                plural: "users".to_string(),
                singular: "user".to_string(),
                needs_mut: true,
            }],
            routes: vec![],
        }
    }

    #[test]
    fn test_html_emitter_contains_page_function() {
        let output = emit(&test_table());
        assert!(output.contains("pub fn html_page("));
        assert!(output.contains("pub fn html_nav("));
        assert!(output.contains("pub fn html_table("));
        assert!(output.contains("pub fn html_form("));
        assert!(output.contains("pub fn html_alert("));
    }

    #[test]
    fn test_html_emitter_uses_app_title() {
        let output = emit(&test_table());
        assert!(output.contains("User Service"));
    }

    #[test]
    fn test_html_emitter_has_nav_links() {
        let output = emit(&test_table());
        assert!(output.contains("Users"));
        assert!(output.contains("New User"));
    }
}
