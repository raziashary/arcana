use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use syn::visit::{self, Visit};
use syn::{Expr, ExprMethodCall, Item, Lit};

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReaderQuery {
    owner: String,
    sql: String,
}

fn is_test(attributes: &[syn::Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        (attribute.path().is_ident("cfg")
            && matches!(
                &attribute.meta,
                syn::Meta::List(list) if list.tokens.to_string().replace(' ', "") == "test"
            ))
            || attribute
                .path()
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "test")
    })
}

fn is_sql_method(method: &str) -> bool {
    matches!(method, "prepare" | "prepare_cached" | "execute")
        || matches!(method, "query" | "query_row" | "execute_batch")
}

fn is_sql_expression(expression: &Expr) -> bool {
    match expression {
        Expr::Lit(_) | Expr::Path(_) | Expr::If(_) | Expr::Block(_) => true,
        Expr::Macro(expression) => {
            expression.mac.path.is_ident("format") || expression.mac.path.is_ident("concat")
        }
        Expr::Reference(expression) => is_sql_expression(&expression.expr),
        Expr::Paren(expression) => is_sql_expression(&expression.expr),
        Expr::Group(expression) => is_sql_expression(&expression.expr),
        _ => false,
    }
}

fn contains_read_query(sql: &str) -> bool {
    let first_word = sql
        .trim_start()
        .split(|character: char| !character.is_ascii_alphabetic())
        .next()
        .unwrap_or_default();
    if matches!(first_word.to_ascii_uppercase().as_str(), "SELECT" | "WITH") {
        return true;
    }

    // Capture reads embedded in mutation statements (for example, UPDATE ...
    // WHERE NOT EXISTS (SELECT ...)). The exact SQL fingerprint covers the
    // whole statement so a change to its read predicate cannot bypass review.
    sql.split(|character: char| !character.is_ascii_alphabetic())
        .any(|word| word.eq_ignore_ascii_case("SELECT"))
}

fn contains_migration_read_query(sql: &str) -> bool {
    let lower = sql.to_ascii_lowercase();
    lower.match_indices("select").any(|(index, _)| {
        let before_is_word = index > 0
            && lower[..index]
                .chars()
                .next_back()
                .is_some_and(|character| character.is_ascii_alphanumeric() || character == '_');
        let after = &lower[index + "select".len()..];
        let after_is_word = after
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphanumeric() || character == '_');
        !before_is_word && !after_is_word && !after.trim_start().starts_with("raise")
    })
}

type SqlBindings = HashMap<String, Expr>;

fn string_from_macro(expression: &syn::ExprMacro, constants: &SqlBindings) -> Option<String> {
    if !expression.mac.path.is_ident("format") && !expression.mac.path.is_ident("concat") {
        return None;
    }

    use syn::parse::Parser;
    let arguments = syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated
        .parse2(expression.mac.tokens.clone())
        .ok()?;
    let first = arguments.first()?;
    resolve_sql(first, constants, &mut Vec::new())
}

fn resolve_sql(
    expression: &Expr,
    bindings: &SqlBindings,
    visiting: &mut Vec<String>,
) -> Option<String> {
    match expression {
        Expr::Lit(expression) => match &expression.lit {
            Lit::Str(value) => Some(value.value()),
            _ => None,
        },
        Expr::Path(expression) if expression.qself.is_none() => {
            let name = expression.path.segments.last()?.ident.to_string();
            if visiting.contains(&name) {
                return None;
            }
            let initializer = bindings.get(&name)?;
            visiting.push(name);
            let result = resolve_sql(initializer, bindings, visiting);
            visiting.pop();
            result
        }
        Expr::Reference(expression) => resolve_sql(&expression.expr, bindings, visiting),
        Expr::Paren(expression) => resolve_sql(&expression.expr, bindings, visiting),
        Expr::Group(expression) => resolve_sql(&expression.expr, bindings, visiting),
        Expr::Macro(expression) => string_from_macro(expression, bindings),
        Expr::MethodCall(expression) if expression.method == "replace" => {
            resolve_sql(&expression.receiver, bindings, visiting)
        }
        Expr::MethodCall(expression) if expression.method == "to_string" => {
            resolve_sql(&expression.receiver, bindings, visiting)
        }
        Expr::If(expression) => {
            let then_sql =
                expression
                    .then_branch
                    .stmts
                    .last()
                    .and_then(|statement| match statement {
                        syn::Stmt::Expr(expression, _) => {
                            resolve_sql(expression, bindings, visiting)
                        }
                        _ => None,
                    });
            let else_sql = expression
                .else_branch
                .as_ref()?
                .1
                .as_ref()
                .pipe(|expression| resolve_sql(expression, bindings, visiting));
            match (then_sql, else_sql) {
                (Some(then_sql), Some(else_sql))
                    if contains_read_query(&then_sql) && contains_read_query(&else_sql) =>
                {
                    Some(then_sql)
                }
                _ => None,
            }
        }
        Expr::Block(expression) => {
            expression
                .block
                .stmts
                .last()
                .and_then(|statement| match statement {
                    syn::Stmt::Expr(expression, _) => resolve_sql(expression, bindings, visiting),
                    _ => None,
                })
        }
        _ => None,
    }
}

fn resolve_sql_variants(
    expression: &Expr,
    bindings: &SqlBindings,
    visiting: &mut Vec<String>,
) -> Option<Vec<String>> {
    let mut variants = match expression {
        Expr::Path(expression) if expression.qself.is_none() => {
            let name = expression.path.segments.last()?.ident.to_string();
            if visiting.contains(&name) {
                return None;
            }
            let initializer = bindings.get(&name)?;
            visiting.push(name);
            let variants = resolve_sql_variants(initializer, bindings, visiting);
            visiting.pop();
            variants?
        }
        Expr::If(expression) => {
            let then_expression =
                expression
                    .then_branch
                    .stmts
                    .last()
                    .and_then(|statement| match statement {
                        syn::Stmt::Expr(expression, _) => Some(expression),
                        _ => None,
                    })?;
            let else_expression = expression.else_branch.as_ref()?.1.as_ref();
            let mut variants = resolve_sql_variants(then_expression, bindings, visiting)?;
            variants.extend(resolve_sql_variants(else_expression, bindings, visiting)?);
            variants
        }
        Expr::Match(expression) => {
            let mut variants = Vec::new();
            for arm in &expression.arms {
                if matches!(arm.body.as_ref(), Expr::Continue(_) | Expr::Return(_)) {
                    continue;
                }
                variants.extend(resolve_sql_variants(&arm.body, bindings, visiting)?);
            }
            variants
        }
        Expr::MethodCall(expression) if expression.method == "replace" => {
            if expression.args.len() != 2 {
                return None;
            }
            let from = resolve_sql_variants(&expression.args[0], bindings, visiting)?;
            if from.len() != 1 {
                return None;
            }
            let from = &from[0];
            let replacements = resolve_sql_variants(&expression.args[1], bindings, visiting)?;
            resolve_sql_variants(&expression.receiver, bindings, visiting)?
                .into_iter()
                .flat_map(|template| {
                    replacements
                        .iter()
                        .map(move |replacement| template.replace(from, replacement))
                })
                .collect()
        }
        Expr::Macro(expression) if expression.mac.path.is_ident("concat") => {
            use syn::parse::Parser;
            let arguments = syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated
                .parse2(expression.mac.tokens.clone())
                .ok()?;
            let mut variants = vec![String::new()];
            for argument in &arguments {
                let parts = resolve_sql_variants(argument, bindings, visiting)?;
                variants = variants
                    .into_iter()
                    .flat_map(|prefix| parts.iter().map(move |part| format!("{prefix}{part}")))
                    .collect();
            }
            variants
        }
        Expr::Macro(expression) if expression.mac.path.is_ident("format") => {
            use syn::parse::Parser;
            let arguments = syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated
                .parse2(expression.mac.tokens.clone())
                .ok()?;
            let template = resolve_sql_variants(arguments.first()?, bindings, visiting)?;
            let mut variants = template;
            for argument in arguments.iter().skip(1) {
                if let Expr::Assign(assignment) = argument {
                    let Expr::Path(name) = assignment.left.as_ref() else {
                        return None;
                    };
                    let key = name.path.segments.last()?.ident.to_string();
                    let values = resolve_sql_variants(&assignment.right, bindings, visiting)?;
                    variants = substitute_format_marker(variants, &format!("{{{key}}}"), values)?;
                } else {
                    let values = resolve_sql_variants(argument, bindings, visiting)?;
                    variants = substitute_format_marker(variants, "{}", values)?;
                }
            }
            // Rust format! also supports implicit named captures, for example
            // `{completed_at_sql}`. Resolve only simple identifiers that are
            // bound to concrete SQL strings; unknown/runtime SQL fragments
            // remain unresolved and fail closed.
            loop {
                let marker = variants.iter().find_map(|template| {
                    let start = template.find('{')?;
                    let end = template[start + 1..].find('}')? + start + 1;
                    let key = &template[start + 1..end];
                    (!key.is_empty()
                        && key
                            .chars()
                            .all(|character| character.is_ascii_alphanumeric() || character == '_')
                        && !key.chars().next()?.is_ascii_digit())
                    .then(|| (format!("{{{key}}}"), key.to_string()))
                });
                let Some((marker, key)) = marker else { break };
                let expression = bindings.get(&key)?;
                let values = resolve_sql_variants(expression, bindings, visiting)?;
                variants = substitute_format_marker(variants, &marker, values)?;
            }
            if variants
                .iter()
                .any(|template| template.contains('{') || template.contains('}'))
            {
                return None;
            }
            variants
        }
        Expr::Reference(expression) => resolve_sql_variants(&expression.expr, bindings, visiting)?,
        Expr::Paren(expression) => resolve_sql_variants(&expression.expr, bindings, visiting)?,
        Expr::Group(expression) => resolve_sql_variants(&expression.expr, bindings, visiting)?,
        _ => vec![resolve_sql(expression, bindings, visiting)?],
    };
    variants.sort();
    variants.dedup();
    Some(variants)
}

fn substitute_format_marker(
    variants: Vec<String>,
    marker: &str,
    replacements: Vec<String>,
) -> Option<Vec<String>> {
    if replacements.is_empty() || !variants.iter().any(|template| template.contains(marker)) {
        return None;
    }
    Some(
        variants
            .into_iter()
            .flat_map(|template| {
                replacements
                    .iter()
                    .map(move |replacement| template.replacen(marker, replacement, 1))
            })
            .collect(),
    )
}

// Avoid depending on a helper trait just to unwrap an optional branch.
trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}
impl<T> Pipe for T {}

#[derive(Default)]
struct ReaderCalls {
    owner: String,
    bindings: SqlBindings,
    queries: Vec<ReaderQuery>,
    unresolved: Vec<String>,
    prepared_statements: std::collections::HashSet<String>,
}

fn receiver_root_name(expression: &Expr) -> Option<String> {
    match expression {
        Expr::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        Expr::Field(field) => receiver_root_name(&field.base),
        Expr::MethodCall(call) => receiver_root_name(&call.receiver),
        Expr::Reference(reference) => receiver_root_name(&reference.expr),
        Expr::Paren(expression) => receiver_root_name(&expression.expr),
        Expr::Group(expression) => receiver_root_name(&expression.expr),
        _ => None,
    }
}

fn is_known_non_read_backup_batch(owner: &str, expression: &Expr, bindings: &SqlBindings) -> bool {
    owner.ends_with("::backup_database_in")
        && resolve_sql(expression, bindings, &mut Vec::new())
            .is_some_and(|sql| sql == "VACUUM INTO '{escaped}'")
}

fn is_prepare_expression(expression: &Expr) -> bool {
    match expression {
        Expr::MethodCall(call)
            if matches!(
                call.method.to_string().as_str(),
                "prepare" | "prepare_cached"
            ) =>
        {
            true
        }
        Expr::MethodCall(call)
            if matches!(
                call.method.to_string().as_str(),
                "map_err" | "unwrap" | "expect"
            ) =>
        {
            is_prepare_expression(&call.receiver)
        }
        Expr::Try(expression) => is_prepare_expression(&expression.expr),
        Expr::Reference(expression) => is_prepare_expression(&expression.expr),
        Expr::Paren(expression) => is_prepare_expression(&expression.expr),
        Expr::Group(expression) => is_prepare_expression(&expression.expr),
        _ => false,
    }
}

impl<'ast> Visit<'ast> for ReaderCalls {
    fn visit_local(&mut self, local: &'ast syn::Local) {
        if let (syn::Pat::Ident(pattern), Some(initializer)) = (&local.pat, &local.init) {
            self.bindings
                .insert(pattern.ident.to_string(), (*initializer.expr).clone());
        }
        visit::visit_local(self, local);
    }

    fn visit_expr_method_call(&mut self, expression: &'ast ExprMethodCall) {
        let method = expression.method.to_string();
        if let Some(sql_expression) = expression.args.first() {
            if is_sql_method(&method) {
                if method == "execute"
                    && receiver_root_name(&expression.receiver)
                        .is_some_and(|name| self.prepared_statements.contains(&name))
                {
                    // Statement::execute receives bind parameters, not SQL.
                    visit::visit_expr_method_call(self, expression);
                    return;
                }
                if matches!(method.as_str(), "query" | "query_row")
                    && !is_sql_expression(sql_expression)
                {
                    // Prepared Statement readers take bound parameters here.
                    // Unknown receivers are unresolved SQL readers and fail
                    // closed instead of being silently skipped.
                    let is_prepared = receiver_root_name(&expression.receiver)
                        .is_some_and(|name| self.prepared_statements.contains(&name));
                    if !is_prepared {
                        self.unresolved.push(format!(
                            "{} uses .{} with unsupported SQL expression",
                            self.owner, method
                        ));
                    }
                    visit::visit_expr_method_call(self, expression);
                    return;
                }
                if !is_sql_expression(sql_expression) {
                    self.unresolved.push(format!(
                        "{} uses .{} with unsupported SQL expression",
                        self.owner, method
                    ));
                } else {
                    match resolve_sql_variants(sql_expression, &self.bindings, &mut Vec::new()) {
                        Some(variants) => self.queries.extend(
                            variants
                                .into_iter()
                                .filter(|sql| contains_read_query(sql))
                                .map(|sql| ReaderQuery {
                                    owner: self.owner.clone(),
                                    sql,
                                }),
                        ),
                        None if method == "execute_batch"
                            && is_known_non_read_backup_batch(
                                &self.owner,
                                sql_expression,
                                &self.bindings,
                            ) => {}
                        None => self.unresolved.push(format!(
                            "{} uses .{} with unresolved SQL argument",
                            self.owner, method
                        )),
                    }
                }
            }
        }
        visit::visit_expr_method_call(self, expression);
    }
}

fn const_bindings(items: &[Item], bindings: &mut SqlBindings) {
    for item in items {
        match item {
            Item::Const(item) if !is_test(&item.attrs) => {
                bindings.insert(item.ident.to_string(), (*item.expr).clone());
            }
            Item::Mod(module) if !is_test(&module.attrs) => {
                if let Some((_, contents)) = &module.content {
                    const_bindings(contents, bindings);
                }
            }
            _ => {}
        }
    }
}

fn scan_items(
    items: &[Item],
    module: &str,
    inherited_test: bool,
    inherited_constants: &SqlBindings,
    output: &mut Vec<ReaderQuery>,
    errors: &mut Vec<String>,
) {
    let mut constants = inherited_constants.clone();
    const_bindings(items, &mut constants);

    for item in items {
        match item {
            Item::Mod(module_item) => {
                if let Some((_, contents)) = &module_item.content {
                    scan_items(
                        contents,
                        &format!("{module}::{}", module_item.ident),
                        inherited_test || is_test(&module_item.attrs),
                        &constants,
                        output,
                        errors,
                    );
                }
            }
            Item::Fn(function) if !inherited_test && !is_test(&function.attrs) => {
                scan_function(
                    &format!("{module}::{}", function.sig.ident),
                    &function.block,
                    &constants,
                    output,
                    errors,
                );
            }
            Item::Impl(implementation) if !inherited_test && !is_test(&implementation.attrs) => {
                let type_name = match implementation.self_ty.as_ref() {
                    syn::Type::Path(path) => path
                        .path
                        .segments
                        .last()
                        .map(|segment| segment.ident.to_string()),
                    _ => None,
                }
                .unwrap_or_else(|| "<unknown>".to_owned());
                for item in &implementation.items {
                    if let syn::ImplItem::Fn(function) = item {
                        if is_test(&function.attrs) {
                            continue;
                        }
                        scan_function(
                            &format!("{type_name}::{}", function.sig.ident),
                            &function.block,
                            &constants,
                            output,
                            errors,
                        );
                    }
                }
            }
            _ => {}
        }
    }
}

fn scan_function(
    owner: &str,
    block: &syn::Block,
    constants: &SqlBindings,
    output: &mut Vec<ReaderQuery>,
    errors: &mut Vec<String>,
) {
    let mut bindings = constants.clone();
    let mut locals = LocalBindings::default();
    locals.visit_block(block);
    bindings.extend(locals.0);

    let mut calls = ReaderCalls {
        owner: owner.to_owned(),
        bindings,
        ..ReaderCalls::default()
    };
    calls.prepared_statements = calls
        .bindings
        .iter()
        .filter_map(|(name, expression)| is_prepare_expression(expression).then(|| name.clone()))
        .collect();
    calls.visit_block(block);
    output.extend(calls.queries);
    errors.extend(calls.unresolved);
}

#[derive(Default)]
struct LocalBindings(SqlBindings);

fn bind_pattern(pattern: &syn::Pat, initializer: &Expr, bindings: &mut SqlBindings) {
    match (pattern, initializer) {
        (syn::Pat::Type(pattern), initializer) => bind_pattern(&pattern.pat, initializer, bindings),
        (syn::Pat::Ident(pattern), initializer) => {
            bindings.insert(pattern.ident.to_string(), initializer.clone());
        }
        (syn::Pat::Tuple(pattern), Expr::Tuple(initializer)) => {
            for (pattern, initializer) in pattern.elems.iter().zip(&initializer.elems) {
                bind_pattern(pattern, initializer, bindings);
            }
        }
        (syn::Pat::Tuple(pattern), Expr::If(initializer)) => {
            let then_tuple =
                initializer
                    .then_branch
                    .stmts
                    .last()
                    .and_then(|statement| match statement {
                        syn::Stmt::Expr(Expr::Tuple(tuple), _) => Some(tuple),
                        _ => None,
                    });
            let else_tuple = initializer
                .else_branch
                .as_ref()
                .and_then(|(_, expression)| tuple_expression(expression));
            if let (Some(then_tuple), Some(else_tuple)) = (then_tuple, else_tuple) {
                for (index, pattern) in pattern.elems.iter().enumerate() {
                    if let (Some(then_expression), Some(else_expression)) =
                        (then_tuple.elems.get(index), else_tuple.elems.get(index))
                    {
                        let condition = &initializer.cond;
                        let then_expression = then_expression;
                        let else_expression = else_expression;
                        let branch: Expr = syn::parse_quote! {
                            if #condition { #then_expression } else { #else_expression }
                        };
                        bind_pattern(pattern, &branch, bindings);
                    }
                }
            }
        }
        (syn::Pat::Tuple(pattern), Expr::Match(initializer)) => {
            for (index, pattern) in pattern.elems.iter().enumerate() {
                let arms = initializer
                    .arms
                    .iter()
                    .map(|arm| {
                        let mut arm = arm.clone();
                        let value = tuple_expression(&arm.body)?.elems.get(index)?.clone();
                        arm.body = Box::new(value);
                        Some(arm)
                    })
                    .collect::<Option<Vec<_>>>();
                if let Some(arms) = arms {
                    let branch = Expr::Match(syn::ExprMatch {
                        attrs: initializer.attrs.clone(),
                        match_token: initializer.match_token,
                        expr: initializer.expr.clone(),
                        brace_token: initializer.brace_token,
                        arms: arms.into_iter().collect(),
                    });
                    bind_pattern(pattern, &branch, bindings);
                }
            }
        }
        _ => {}
    }
}

fn tuple_expression(expression: &Expr) -> Option<&syn::ExprTuple> {
    match expression {
        Expr::Tuple(tuple) => Some(tuple),
        Expr::Block(block) => block
            .block
            .stmts
            .last()
            .and_then(|statement| match statement {
                syn::Stmt::Expr(expression, _) => tuple_expression(expression),
                _ => None,
            }),
        Expr::Paren(expression) => tuple_expression(&expression.expr),
        Expr::Group(expression) => tuple_expression(&expression.expr),
        _ => None,
    }
}

impl<'ast> Visit<'ast> for LocalBindings {
    fn visit_local(&mut self, local: &'ast syn::Local) {
        if let Some(initializer) = &local.init {
            bind_pattern(&local.pat, &initializer.expr, &mut self.0);
        }
        visit::visit_local(self, local);
    }
}

fn scan_source(source: &str, module: &str) -> (Vec<ReaderQuery>, Vec<String>) {
    scan_source_with(source, module, &SqlBindings::new())
}

fn scan_source_with(
    source: &str,
    module: &str,
    inherited_constants: &SqlBindings,
) -> (Vec<ReaderQuery>, Vec<String>) {
    let file = syn::parse_file(source)
        .unwrap_or_else(|error| panic!("{module} Rust source did not parse: {error}"));
    if is_test(&file.attrs) {
        return (Vec::new(), Vec::new());
    }
    let mut constants = inherited_constants.clone();
    const_bindings(&file.items, &mut constants);
    let mut queries = Vec::new();
    let mut errors = Vec::new();
    scan_items(
        &file.items,
        module,
        false,
        &constants,
        &mut queries,
        &mut errors,
    );
    (queries, errors)
}

fn scoped_source_roots(manifest_dir: &Path) -> Vec<(&'static str, PathBuf)> {
    [("core_db", "src/db"), ("analytics", "src/analytics")]
        .into_iter()
        .map(|(scope, relative)| (scope, manifest_dir.join(relative)))
        .chain(std::iter::once((
            "shared_analytics",
            manifest_dir.join("../src-shared/src/db"),
        )))
        .collect()
}

fn rust_sources_under(root: &Path) -> Vec<PathBuf> {
    let mut sources = walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .map(|entry| {
            entry.unwrap_or_else(|error| {
                panic!("cannot enumerate scoped production source tree {root:?}: {error}")
            })
        })
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "rs"))
        .map(|entry| entry.into_path())
        .collect::<Vec<_>>();
    sources.sort();
    sources
}

fn production_sources() -> Vec<(String, String)> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    const CORE_DB_SCOPE_EXCEPTIONS: &[&str] = &[
        // These are settings/IDE annotation stores, not Fleet, dashboard,
        // dispatch, recovery, task lifecycle, observability, or CLI analytics.
        "src/db/annotations.rs",
        "src/db/app_settings.rs",
        "src/db/arcana_instructions.rs",
        "src/db/model_configs.rs",
        // Schema helpers and migration DDL are not runtime reader queries;
        // task DB migration SELECTs are separately inspected below.
        "src/db/migrate.rs",
    ];
    let mut sources = Vec::new();
    for (scope, root) in scoped_source_roots(manifest_dir) {
        for path in rust_sources_under(&root) {
            let relative = path
                .strip_prefix(manifest_dir)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            if scope == "core_db"
                && CORE_DB_SCOPE_EXCEPTIONS
                    .iter()
                    .any(|exception| relative == *exception)
            {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
                panic!("cannot read scoped production source {path:?}: {error}")
            });
            sources.push((format!("{scope}::{relative}"), source));
        }
    }
    sources
}

fn production_queries() -> (Vec<ReaderQuery>, Vec<String>) {
    let sources = production_sources();
    let mut constants = SqlBindings::new();
    for (_, source) in &sources {
        let file = syn::parse_file(source).expect("scoped production Rust source parses");
        const_bindings(&file.items, &mut constants);
    }
    let mut queries = Vec::new();
    let mut errors = Vec::new();
    for (module, source) in &sources {
        let (mut found, mut unresolved) = scan_source_with(source, module, &constants);
        if module.starts_with("shared_analytics::") {
            for query in &mut found {
                query.owner = format!("Shared::{}", query.owner);
            }
        }
        queries.append(&mut found);
        errors.append(&mut unresolved);
    }
    for (module, source) in &sources {
        let migration_owner = match module.rsplit("/").next().unwrap_or_default() {
            "tasks.rs" => "TasksDb".to_owned(),
            "workers.rs" => "WorkersDb".to_owned(),
            "runs.rs" => "RunsDb".to_owned(),
            "plans.rs" => "PlansDb".to_owned(),
            "db.rs" if module.starts_with("analytics::") => "AnalyticsDb".to_owned(),
            _ => module.clone(),
        };
        let (mut migration_queries, mut migration_errors) =
            scan_migration_sql(source, &migration_owner);
        queries.append(&mut migration_queries);
        errors.append(&mut migration_errors);
    }
    (queries, errors)
}

#[test]
fn scoped_source_discovery_includes_new_production_database_modules() {
    let fixture = tempfile::tempdir().expect("temporary source tree");
    let nested = fixture.path().join("src/db/new_reader.rs");
    std::fs::create_dir_all(nested.parent().expect("nested module parent"))
        .expect("create nested module directory");
    std::fs::write(&nested, "pub fn new_reader() {}").expect("write source fixture");

    let discovered = rust_sources_under(&fixture.path().join("src/db"));
    assert_eq!(discovered, vec![nested]);
}

#[test]
fn query_inventory_documents_each_source_scope_exception() {
    let inventory =
        include_str!("../../docs/restructurization/development/_verify/P4-58-QUERY-INVENTORY.md");
    for excluded in [
        "src/db/annotations.rs",
        "src/db/app_settings.rs",
        "src/db/arcana_instructions.rs",
        "src/db/model_configs.rs",
        "src/db/migrate.rs",
    ] {
        assert!(
            inventory.contains(excluded),
            "the P4-58 inventory must document the scope exception for {excluded}"
        );
    }
}

#[derive(Default)]
struct MigrationSqlScan {
    owner: String,
    queries: Vec<ReaderQuery>,
}

impl<'ast> Visit<'ast> for MigrationSqlScan {
    fn visit_item_const(&mut self, item: &'ast syn::ItemConst) {
        if !is_test(&item.attrs) {
            self.visit_expr(&item.expr);
        }
    }

    fn visit_expr_struct(&mut self, expression: &'ast syn::ExprStruct) {
        if expression
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "Migration")
        {
            let version = expression.fields.iter().find_map(|field| {
                matches!(&field.member, syn::Member::Named(name) if name == "version")
                    .then(|| match &field.expr {
                        Expr::Lit(value) => match &value.lit {
                            Lit::Int(value) => Some(value.base10_digits().to_owned()),
                            _ => None,
                        },
                        _ => None,
                    })
                    .flatten()
            });
            let name = expression.fields.iter().find_map(|field| {
                matches!(&field.member, syn::Member::Named(name) if name == "name")
                    .then(|| match &field.expr {
                        Expr::Lit(value) => match &value.lit {
                            Lit::Str(value) => Some(value.value()),
                            _ => None,
                        },
                        _ => None,
                    })
                    .flatten()
            });
            let sql = expression.fields.iter().find_map(|field| {
                matches!(&field.member, syn::Member::Named(name) if name == "sql")
                    .then(|| resolve_sql(&field.expr, &SqlBindings::new(), &mut Vec::new()))
                    .flatten()
            });
            if let (Some(version), Some(name), Some(sql)) = (version, name, sql) {
                if contains_migration_read_query(&sql) {
                    self.queries.push(ReaderQuery {
                        owner: format!("{}::migration/{version}:{name}", self.owner),
                        sql,
                    });
                }
            }
        }
        visit::visit_expr_struct(self, expression);
    }
}

fn scan_migration_sql(source: &str, owner: &str) -> (Vec<ReaderQuery>, Vec<String>) {
    let file = syn::parse_file(source)
        .unwrap_or_else(|error| panic!("{owner} migration source did not parse: {error}"));
    let mut scan = MigrationSqlScan {
        owner: owner.to_owned(),
        ..MigrationSqlScan::default()
    };
    scan.visit_file(&file);
    (scan.queries, Vec::new())
}

fn query_fingerprint(sql: &str) -> String {
    let normalized = sql
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    hex::encode(Sha256::digest(normalized.as_bytes()))
}

#[test]
// This guards source registration only; populated-fixture EXPLAIN assertions
// remain the responsibility of the per-query plan regressions.
fn production_readers_are_registered_in_the_p4_query_inventory() {
    let inventory =
        include_str!("../../docs/restructurization/development/_verify/P4-58-QUERY-INVENTORY.md");
    let (queries, unresolved) = production_queries();

    assert!(
        !queries.is_empty(),
        "the production scanner found no SELECT/WITH readers"
    );

    let registry = inventory
        .split("### Exact production query fingerprints")
        .nth(1)
        .unwrap_or_else(|| panic!("P4-58 inventory is missing the exact-query registry section"));
    let registered = registry
        .lines()
        .filter_map(|line| {
            let cells = line
                .trim()
                .strip_prefix('|')?
                .strip_suffix('|')?
                .split('|')
                .map(str::trim)
                .collect::<Vec<_>>();
            if cells.len() < 3
                || cells[0] == "Production owner"
                || cells[0].chars().all(|character| character == '-')
            {
                return None;
            }
            Some((cells[0].replace('`', ""), cells[1].to_string()))
        })
        .collect::<std::collections::HashSet<_>>();
    let discovered = queries
        .iter()
        .map(|query| (query.owner.clone(), query_fingerprint(&query.sql)))
        .collect::<std::collections::HashSet<_>>();
    let mut missing = discovered
        .difference(&registered)
        .map(|(owner, fingerprint)| format!("{owner} | {fingerprint}"))
        .collect::<Vec<_>>();
    missing.sort();
    let mut stale = registered
        .difference(&discovered)
        .map(|(owner, fingerprint)| format!("{owner} | {fingerprint}"))
        .collect::<Vec<_>>();
    stale.sort();
    let mut issues = Vec::new();
    if !unresolved.is_empty() {
        issues.push(format!(
            "unresolved dynamic SQL; add a source-level SQL constant or make SQL statically resolvable:\n{}",
            unresolved.join("\n")
        ));
    }
    if !missing.is_empty() {
        issues.push(format!(
            "production SQL fingerprints are missing from the P4-58 registry; add each query with plan evidence or an explicit scope decision:\n{}",
            missing.join("\n")
        ));
    }
    if !stale.is_empty() {
        issues.push(format!(
            "P4-58 fingerprints no longer exist in scoped production sources; remove or reconcile stale entries:\n{}",
            stale.join("\n")
        ));
    }
    assert!(issues.is_empty(), "{}", issues.join("\n\n"));
}

#[test]
fn every_discovered_reader_owner_has_a_proof_reference_or_reasoned_exclusion() {
    let inventory =
        include_str!("../../docs/restructurization/development/_verify/P4-58-QUERY-INVENTORY.md");
    let owner_section = inventory
        .split("## AST-discovered source owners")
        .nth(1)
        .unwrap_or_else(|| panic!("P4-58 inventory is missing its discovered-owner section"))
        .split("## Explicit exclusions to verify")
        .next()
        .expect("owner inventory section boundary");
    let evidence_by_owner = owner_section
        .lines()
        .filter_map(|line| {
            let cells = line
                .trim()
                .strip_prefix('|')?
                .strip_suffix('|')?
                .split('|')
                .map(str::trim)
                .collect::<Vec<_>>();
            if cells.len() != 3 || cells[0].starts_with('-') {
                return None;
            }
            Some((cells[0].replace('`', ""), cells[2].to_owned()))
        })
        .collect::<HashMap<_, _>>();
    let fingerprint_section = inventory
        .split("### Exact production query fingerprints")
        .nth(1)
        .unwrap_or_else(|| panic!("P4-58 inventory is missing its exact fingerprint section"));
    let mut fingerprint_evidence_issues = Vec::new();
    for line in fingerprint_section.lines() {
        let Some(cells) = line
            .trim()
            .strip_prefix('|')
            .and_then(|line| line.strip_suffix('|'))
            .map(|line| line.split('|').map(str::trim).collect::<Vec<_>>())
        else {
            continue;
        };
        if cells.len() != 3
            || cells[1].len() != 64
            || !cells[1].chars().all(|c| c.is_ascii_hexdigit())
        {
            continue;
        }
        let owner = cells[0].replace('`', "");
        match evidence_by_owner.get(&owner) {
            Some(owner_evidence)
                if !cells[2].contains("See owner inventory row above")
                    && (owner_evidence
                        .split('`')
                        .skip(1)
                        .step_by(2)
                        .any(|reference| cells[2].contains(reference))
                        || [
                            "bounded exclusion",
                            "not a dashboard reader",
                            "not a dashboard/dispatch reader",
                            "no production caller",
                            "startup write/seed path",
                            "intermediate migration-only",
                            "explicit bounded full-copy",
                        ]
                        .iter()
                        .any(|phrase| cells[2].to_ascii_lowercase().contains(phrase))) => {}
            Some(_) => fingerprint_evidence_issues.push(format!(
                "{owner} {}: fingerprint row lacks its direct proof/rationale text",
                cells[1]
            )),
            None => fingerprint_evidence_issues.push(format!(
                "{owner} {}: fingerprint has no matching owner evidence",
                cells[1]
            )),
        }
    }
    let (queries, unresolved) = production_queries();
    assert!(unresolved.is_empty(), "unresolved SQL: {unresolved:?}");

    let owners = queries
        .iter()
        .map(|query| query.owner.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let mut missing = Vec::new();
    for owner in owners {
        let Some(evidence) = evidence_by_owner.get(owner) else {
            missing.push(format!("{owner}: missing owner evidence row"));
            continue;
        };
        let lower = evidence.to_ascii_lowercase();
        let has_proof_reference = evidence.matches('`').count() >= 2;
        let has_reasoned_exclusion = [
            "bounded exclusion",
            "not a dashboard reader",
            "not a dashboard/dispatch reader",
            "no production caller",
            "startup write/seed path",
            "intermediate migration-only",
            "explicit bounded full-copy",
        ]
        .iter()
        .any(|phrase| lower.contains(phrase));
        if evidence.trim().len() < 24 || (!has_proof_reference && !has_reasoned_exclusion) {
            missing.push(format!(
                "{owner}: no named proof reference or reasoned exclusion"
            ));
        }
    }
    assert!(
        missing.is_empty() && fingerprint_evidence_issues.is_empty(),
        "every discovered owner and each exact fingerprint needs direct populated plan/result evidence or an explicit scope/contract rationale:\n{}\n{}",
        missing.join("\n"),
        fingerprint_evidence_issues.join("\n")
    );
}

#[test]
fn ast_scanner_resolves_multiline_with_constants_and_variables_and_ignores_tests_and_comments() {
    let source = concat!(
        "const BASE: &str = \"WITH recent AS (\\n SELECT id FROM tasks\\n) SELECT id FROM recent\";\n",
        "struct Db;\n",
        "impl Db {\n",
        "  fn new_reader(&self) {\n",
        "    let sql = BASE;\n",
        "    let _ = self.conn.query_row(sql, [], |_| Ok(()));\n",
        "  }\n",
        "}\n",
        "#[cfg(test)] mod tests {\n",
        "  fn test_helper() { let _ = conn.query_row(\"SELECT test_only FROM tasks\", [], |_| Ok(())); }\n",
        "}\n",
        "// conn.query_row(\"SELECT comment_only FROM tasks\", [], |_| Ok(()));\n",
    );

    let (queries, errors) = scan_source(source, "synthetic");
    assert!(errors.is_empty(), "unexpected scanner errors: {errors:?}");
    assert_eq!(
        queries,
        vec![ReaderQuery {
            owner: "Db::new_reader".into(),
            sql: "WITH recent AS (\n SELECT id FROM tasks\n) SELECT id FROM recent".into(),
        }]
    );
}

#[test]
fn ast_scanner_ignores_explicit_test_only_files_but_checks_runtime_files() {
    let body = "fn fixture() { conn.query_row(dynamic_sql, [], |_| Ok(())); }";
    let (queries, errors) = scan_source(&format!("#![cfg(test)]\n{body}"), "test_fixture");
    assert!(queries.is_empty() && errors.is_empty());
    let (_, errors) = scan_source(body, "runtime");
    assert_eq!(
        errors.len(),
        1,
        "unknown runtime SQL must still fail the registry"
    );
}

#[test]
fn ast_scanner_resolves_finite_replace_variants_and_execute_batch_reads() {
    let source = concat!(
        "const TEMPLATE: &str = \"SELECT id FROM tasks WHERE 1=1{filter}\";\n",
        "fn reader(enabled: bool) {\n",
        "  let filter = if enabled { \" AND status = ?1\" } else { \"\" };\n",
        "  let sql = TEMPLATE.replace(\"{filter}\", filter);\n",
        "  let _ = conn.prepare(&sql);\n",
        "  conn.execute_batch(\"UPDATE tasks SET status = 'ready' WHERE id = (SELECT MIN(id) FROM tasks)\");\n",
        "}\n",
    );

    let (queries, errors) = scan_source(source, "synthetic");
    assert!(errors.is_empty(), "unexpected scanner errors: {errors:?}");
    assert_eq!(
        queries,
        vec![
            ReaderQuery {
                owner: "synthetic::reader".into(),
                sql: "SELECT id FROM tasks WHERE 1=1".into(),
            },
            ReaderQuery {
                owner: "synthetic::reader".into(),
                sql: "SELECT id FROM tasks WHERE 1=1 AND status = ?1".into(),
            },
            ReaderQuery {
                owner: "synthetic::reader".into(),
                sql: "UPDATE tasks SET status = 'ready' WHERE id = (SELECT MIN(id) FROM tasks)"
                    .into(),
            },
        ]
    );
}

#[test]
fn ast_scanner_rejects_unresolved_execute_batch_sql_fragments() {
    let source = concat!(
        "fn migration(conn: &Connection, predicate: &str) {\n",
        "  let sql = format!(\"UPDATE tasks SET status = 'ready' WHERE id IN ({})\", predicate);\n",
        "  conn.execute_batch(&sql);\n",
        "}\n",
    );

    let (queries, errors) = scan_source(source, "synthetic");
    assert!(
        queries.is_empty(),
        "unresolved SQL must not be fingerprinted as concrete: {queries:?}"
    );
    assert_eq!(
        errors.len(),
        1,
        "unknown execute_batch SQL must fail closed: {errors:?}"
    );
    assert!(errors[0].contains("unresolved SQL argument"));
}

#[test]
fn ast_scanner_allows_only_the_fingerprinted_vacuum_backup_batch_exception() {
    let source = concat!(
        "fn backup_database_in(conn: &Connection, escaped: &str) {\n",
        "  let sql = format!(\"VACUUM INTO '{escaped}'\");\n",
        "  conn.execute_batch(&sql);\n",
        "}\n",
    );

    let (queries, errors) = scan_source(source, "core_db::src/db/backup.rs");
    assert!(
        queries.is_empty(),
        "VACUUM is outside the read-plan inventory"
    );
    assert!(
        errors.is_empty(),
        "the single documented VACUUM path is a non-read exception: {errors:?}"
    );
}
