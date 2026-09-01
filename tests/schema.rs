//! Contract tests for the published planr schema.
//!
//! Three things are checked here: that the schema document is itself a valid
//! draft 2020-12 schema, that its `$id` still agrees with the path it is
//! published at, and that a corpus of fixtures is accepted or rejected exactly
//! as intended. Fixtures are YAML because that is what a project actually
//! writes; each invalid one carries a comment saying why it must fail.
//!
//! These run under `cargo test`, so the CI pipeline needs no separate job.

use std::fs;
use std::path::PathBuf;

use boon::{Compiler, SchemaIndex, Schemas};
use serde_json::Value;

const SCHEMA_PATH: &str = "schemas/planr/v1/1.0.0/planr.schema.json";
const SCHEMA_ID: &str = "https://schemas.columnzero.com/planr/v1/1.0.0/planr.schema.json";
const METASCHEMA: &str = "https://json-schema.org/draft/2020-12/schema";
const FIXTURE_ROOT: &str = "tests/fixtures/schema";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn schema_doc() -> Value {
    let path = repo_root().join(SCHEMA_PATH);
    let text =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{SCHEMA_PATH} is not valid JSON: {e}"))
}

/// The three published entry points: the root schema for `.plan/schema.yml`,
/// and the `#ticket` and `#commit` anchors inside the same document.
struct Targets {
    schemas: Schemas,
    root: SchemaIndex,
    ticket: SchemaIndex,
    commit: SchemaIndex,
}

impl Targets {
    fn compile() -> Self {
        let mut schemas = Schemas::new();
        let mut compiler = Compiler::new();
        compiler
            .add_resource(SCHEMA_ID, schema_doc())
            .expect("schema document is not a usable JSON Schema resource");

        let root = compiler
            .compile(SCHEMA_ID, &mut schemas)
            .expect("root schema failed to compile");
        let ticket = compiler
            .compile(&format!("{SCHEMA_ID}#ticket"), &mut schemas)
            .expect("#ticket anchor failed to compile");
        let commit = compiler
            .compile(&format!("{SCHEMA_ID}#commit"), &mut schemas)
            .expect("#commit anchor failed to compile");

        Targets {
            schemas,
            root,
            ticket,
            commit,
        }
    }

    fn get(&self, name: &str) -> SchemaIndex {
        match name {
            "root" => self.root,
            "ticket" => self.ticket,
            "commit" => self.commit,
            other => panic!("no such schema target: {other}"),
        }
    }

    fn accepts(&self, target: &str, instance: &Value) -> Result<(), String> {
        self.schemas
            .validate(instance, self.get(target))
            .map_err(|e| e.to_string())
    }
}

/// Load every `.yml` fixture under `tests/fixtures/schema/<target>/<bucket>`,
/// sorted so failures report in a stable order.
fn fixtures(target: &str, bucket: &str) -> Vec<(String, Value)> {
    let dir = repo_root().join(FIXTURE_ROOT).join(target).join(bucket);
    let entries =
        fs::read_dir(&dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()));

    let mut loaded = Vec::new();
    for entry in entries {
        let path = entry.expect("cannot stat fixture").path();
        if path.extension().and_then(|s| s.to_str()) != Some("yml") {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("fixture has no readable name")
            .to_string();
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let value = serde_yaml::from_str::<Value>(&text)
            .unwrap_or_else(|e| panic!("{target}/{bucket}/{name}.yml is not valid YAML: {e}"));
        loaded.push((name, value));
    }

    assert!(
        !loaded.is_empty(),
        "no fixtures found in {} -- the corpus is how this contract is pinned",
        dir.display()
    );
    loaded.sort_by(|a, b| a.0.cmp(&b.0));
    loaded
}

#[test]
fn schema_document_is_valid_draft_2020_12() {
    let mut schemas = Schemas::new();
    let mut compiler = Compiler::new();
    let meta = compiler
        .compile(METASCHEMA, &mut schemas)
        .expect("draft 2020-12 metaschema is bundled with the validator");

    if let Err(e) = schemas.validate(&schema_doc(), meta) {
        panic!("{SCHEMA_PATH} is not a valid draft 2020-12 schema:\n{e}");
    }
}

#[test]
fn schema_id_matches_its_published_path() {
    let doc = schema_doc();
    let id = doc
        .get("$id")
        .and_then(Value::as_str)
        .expect("schema document has no $id");

    // The in-tree layout mirrors what the registry serves, and $id is the
    // CANONICAL versioned URL -- canonical URLs never move, so this pins the
    // document's permanent identity. Projects cite the alias
    // (.../planr/v1/planr.schema.json) instead, which moves forward with each
    // release; that is why $id must not be alias-shaped.
    let expected = format!(
        "https://schemas.columnzero.com/{}",
        SCHEMA_PATH.trim_start_matches("schemas/")
    );
    assert_eq!(
        id, expected,
        "$id and the in-tree path have diverged; the registry URL would break"
    );
    assert_eq!(id, SCHEMA_ID, "$id changed without updating the test");
}

#[test]
fn schema_compiles_and_exposes_its_anchors() {
    // Compiling all three targets is the check: a missing or renamed $anchor
    // means `planr.schema.json#ticket` stops resolving for outside tooling.
    Targets::compile();
}

#[test]
fn fixtures_validate_as_intended() {
    let targets = Targets::compile();
    let mut failures: Vec<String> = Vec::new();
    let mut checked = 0usize;

    for target in ["root", "ticket", "commit"] {
        for (name, instance) in fixtures(target, "valid") {
            checked += 1;
            if let Err(e) = targets.accepts(target, &instance) {
                failures.push(format!("{target}/valid/{name} was rejected:\n{e}"));
            }
        }
        for (name, instance) in fixtures(target, "invalid") {
            checked += 1;
            if targets.accepts(target, &instance).is_ok() {
                failures.push(format!(
                    "{target}/invalid/{name} was accepted, but the fixture exists to be rejected"
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} of {checked} fixtures did not behave as intended:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

#[test]
fn the_reference_schema_validates_against_the_published_schema() {
    // The drift-catcher. Every other test here validates FIXTURES, which are
    // written to match the published schema and so can never disagree with
    // it. Nothing validated the schema the tool actually loads -- and that is
    // precisely where drift accumulated: the published document kept
    // `effect: delete` and `templates.<kind>.sections` for two renames after
    // the implementation had moved to `ticket-only` and `initial`, and both
    // sides passed their own tests throughout.
    let path = repo_root().join(".plan/schema.yml");
    let text =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let instance = serde_yaml::from_str::<Value>(&text)
        .unwrap_or_else(|e| panic!(".plan/schema.yml is not valid YAML: {e}"));

    let targets = Targets::compile();
    if let Err(e) = targets.accepts("root", &instance) {
        panic!(
            "the reference schema at .plan/schema.yml does not satisfy the published schema.\n\
             One of them is behind the other; reconcile before publishing.\n{e}"
        );
    }
}
