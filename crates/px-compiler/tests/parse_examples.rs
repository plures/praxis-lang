//! Integration test: every `examples/*.px` file must parse cleanly into the
//! canonical AST via `px_compiler::parse`. This is the end-to-end "the front
//! end really works on real programs" gate for the epic (not just unit shapes).

use std::fs;
use std::path::PathBuf;

fn examples_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR = <repo>/crates/px-compiler → examples at <repo>/examples
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
}

fn collect_px_files() -> Vec<PathBuf> {
    let dir = examples_dir();
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read examples dir {}: {e}", dir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map(|x| x == "px").unwrap_or(false))
        .collect();
    files.sort();
    files
}

#[test]
fn examples_dir_has_at_least_three_programs() {
    let files = collect_px_files();
    assert!(
        files.len() >= 3,
        "expected >= 3 example .px programs, found {}: {:?}",
        files.len(),
        files
    );
}

#[test]
fn all_examples_parse() {
    let files = collect_px_files();
    assert!(!files.is_empty(), "no example .px files found");
    for path in &files {
        let src = fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        match px_compiler::parse(&src) {
            Ok(doc) => {
                assert!(
                    !doc.statements.is_empty(),
                    "{} parsed to zero statements",
                    path.display()
                );
            }
            Err(e) => panic!("failed to parse {}:\n{e}", path.display()),
        }
    }
}

#[test]
fn memory_assistant_lowers_expected_constructs() {
    use px_ast::Statement;
    let src = fs::read_to_string(examples_dir().join("memory_assistant.px")).unwrap();
    let doc = px_compiler::parse(&src).expect("memory_assistant.px must parse");

    let mut imports = 0;
    let mut entities = 0;
    let mut facts = 0;
    let mut configs = 0;
    let mut functions = 0;
    let mut rules = 0;
    let mut constraints = 0;
    for s in &doc.statements {
        match s {
            Statement::Import(_) => imports += 1,
            Statement::Entity(_) => entities += 1,
            Statement::Fact(_) => facts += 1,
            Statement::Config(_) => configs += 1,
            Statement::Function(_) => functions += 1,
            Statement::Rule(_) => rules += 1,
            Statement::Constraint(_) => constraints += 1,
            _ => {}
        }
    }
    assert_eq!(imports, 2, "expected 2 imports");
    assert_eq!(entities, 1, "expected 1 entity");
    assert_eq!(facts, 1, "expected 1 fact");
    assert_eq!(configs, 1, "expected 1 config");
    assert_eq!(functions, 1, "expected 1 function");
    assert_eq!(rules, 1, "expected 1 rule");
    assert_eq!(constraints, 2, "expected 2 constraints");
}
