// Python extraction on PEP 8 layout (two blank lines before each top-level
// definition). The patterns used `^(\s*)`, and `\s` matches a newline, so the
// match began at the blank line above the `def`: the captured "indent" was a
// run of newlines, which made every top-level function after a blank line a
// "method" with no CONTAINS edge, and put its `line_start` on the blank line.

use super::extract::extract;
use super::extract::{EdgeKind, NodeKind};

const PEP8_SOURCE: &str = "import os


class Widget:
    def method(self):
        return helper()


def helper():
    return 1


async def runner():
    return helper()
";

#[test]
fn top_level_python_definitions_after_blank_lines_are_top_level_with_true_lines() {
    let dir = tempfile::tempdir().expect("workspace");
    let file = dir.path().join("sample.py");
    std::fs::write(&file, PEP8_SOURCE).expect("write sample");
    let result = extract(&file, dir.path()).expect("extract");
    let key = |name: &str| format!("{}::{name}", file.display());

    let find = |name: &str| {
        result
            .nodes
            .iter()
            .find(|n| n.name == name)
            .unwrap_or_else(|| panic!("no node named {name}"))
    };
    assert_eq!(find("Widget").line_start, 4);
    assert_eq!(find("method").line_start, 5);
    assert_eq!(find("method").modifiers.as_deref(), Some("method"));
    let helper = find("helper");
    assert_eq!(
        helper.line_start, 9,
        "the def line, not the blank line above"
    );
    assert!(
        helper.modifiers.is_none(),
        "a top-level function is not a method"
    );
    assert!(matches!(helper.kind, NodeKind::Function));
    assert_eq!(find("runner").line_start, 13);
    assert!(find("runner").modifiers.is_none());

    let contained: Vec<&str> = result
        .edges
        .iter()
        .filter(|e| matches!(e.kind, EdgeKind::Contains))
        .map(|e| e.target_qualified.as_str())
        .collect();
    for top_level in ["Widget", "helper", "runner"] {
        assert!(
            contained.contains(&key(top_level).as_str()),
            "{top_level} is missing its CONTAINS edge: {contained:?}"
        );
    }
    assert!(
        !contained.contains(&key("method").as_str()),
        "a method is contained by its class, not the file"
    );

    // Calls are attributed to the enclosing definition.
    let calls: Vec<(String, String)> = result
        .edges
        .iter()
        .filter(|e| matches!(e.kind, EdgeKind::Calls))
        .map(|e| (e.source_qualified.clone(), e.target_qualified.clone()))
        .collect();
    assert!(calls.contains(&(key("method"), key("helper"))), "{calls:?}");
    assert!(calls.contains(&(key("runner"), key("helper"))), "{calls:?}");
}
