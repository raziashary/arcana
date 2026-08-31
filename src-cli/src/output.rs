use colored::Colorize;
use comfy_table::{Table, Cell, Attribute, Color};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq)]
pub enum OutputFormat {
    Table,
    Json,
    Plain,
}

#[derive(Clone)]
pub struct OutputContext {
    pub format: OutputFormat,
    pub quiet: bool,
    pub non_interactive: bool,
    pub orchestrator_url: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub working_dir: Option<String>,
}

impl OutputContext {
    pub fn print_table(&self, headers: &[&str], rows: Vec<Vec<String>>) {
        if self.quiet { return; }
        match self.format {
            OutputFormat::Json => {
                let objects: Vec<serde_json::Value> = rows.iter().map(|row| {
                    let mut map = serde_json::Map::new();
                    for (i, h) in headers.iter().enumerate() {
                        map.insert(h.to_string(), serde_json::Value::String(
                            row.get(i).cloned().unwrap_or_default()
                        ));
                    }
                    serde_json::Value::Object(map)
                }).collect();
                println!("{}", serde_json::to_string_pretty(&objects).unwrap_or_default());
            }
            OutputFormat::Plain => {
                for row in &rows {
                    println!("{}", row.join("\t"));
                }
            }
            OutputFormat::Table => {
                let mut table = Table::new();
                table.set_header(
                    headers.iter().map(|h| {
                        Cell::new(h).add_attribute(Attribute::Bold).fg(Color::Cyan)
                    })
                );
                for row in rows {
                    table.add_row(row);
                }
                println!("{table}");
            }
        }
    }

    pub fn print_json<T: Serialize>(&self, value: &T) {
        if self.quiet { return; }
        match self.format {
            OutputFormat::Json | OutputFormat::Table => {
                println!("{}", serde_json::to_string_pretty(value).unwrap_or_default());
            }
            OutputFormat::Plain => {
                println!("{}", serde_json::to_string(value).unwrap_or_default());
            }
        }
    }

    pub fn print_kv(&self, pairs: &[(&str, &str)]) {
        if self.quiet { return; }
        match self.format {
            OutputFormat::Json => {
                let mut map = serde_json::Map::new();
                for (k, v) in pairs {
                    map.insert(k.to_string(), serde_json::Value::String(v.to_string()));
                }
                println!("{}", serde_json::to_string_pretty(&map).unwrap_or_default());
            }
            OutputFormat::Plain => {
                for (k, v) in pairs {
                    println!("{k}={v}");
                }
            }
            OutputFormat::Table => {
                let mut table = Table::new();
                table.set_header(vec![
                    Cell::new("Key").add_attribute(Attribute::Bold).fg(Color::Cyan),
                    Cell::new("Value").add_attribute(Attribute::Bold).fg(Color::Cyan),
                ]);
                for (k, v) in pairs {
                    table.add_row(vec![k.to_string(), v.to_string()]);
                }
                println!("{table}");
            }
        }
    }

    pub fn success(&self, msg: &str) {
        if self.quiet { return; }
        if self.format == OutputFormat::Json {
            println!("{}", serde_json::json!({"status": "ok", "message": msg}));
        } else {
            println!("{}", msg.green());
        }
    }

    pub fn error(&self, msg: &str) {
        if self.format == OutputFormat::Json {
            eprintln!("{}", serde_json::json!({"status": "error", "message": msg}));
        } else {
            eprintln!("{}", msg.red());
        }
    }

    pub fn info(&self, msg: &str) {
        if self.quiet { return; }
        if self.format == OutputFormat::Json { return; }
        println!("{}", msg.dimmed());
    }

    pub fn warn(&self, msg: &str) {
        if self.quiet { return; }
        if self.format == OutputFormat::Json {
            eprintln!("{}", serde_json::json!({"status": "warn", "message": msg}));
        } else {
            eprintln!("{}", msg.yellow());
        }
    }
}

/// Build a status-colored cell string.
pub fn status_colored(status: &str) -> String {
    match status {
        "active" | "completed" | "running" | "healthy" => status.green().to_string(),
        "failed" | "error" | "dead" => status.red().to_string(),
        "pending" | "draft" | "idle" => status.yellow().to_string(),
        _ => status.to_string(),
    }
}
