//! Tests for `config_value_from_yaml` — the structural fix for pest's
//! indentation-blindness on `.px` config blocks.
//!
//! The pest front end can silently absorb a level-2 config sibling as a *child*
//! of the preceding level-2 entry (wrong tree, no error). Config data is exactly
//! a YAML mapping, so parsing the block body through `serde_yaml` yields the
//! correct structure. These tests pin that behavior so it cannot regress.

use px_yaml::config_value_from_yaml;
use serde_json::Value;

/// The canonical sibling case: two level-2 siblings, each with level-3 children.
/// pest absorbs the second sibling; serde_yaml keeps them as peers.
#[test]
fn siblings_are_not_absorbed() {
    let body = "law: never assume\n\
                peer_discovery:\n  \
                pattern: fire_and_forget\n  \
                zero_peers: normal\n\
                cron_sweeps:\n  \
                must_be: idempotent\n  \
                every_action: ledger\n";

    let v = config_value_from_yaml(body).expect("config body must parse");
    let obj = v.as_object().expect("top level is a mapping");

    // Three SIBLINGS at the top level.
    assert_eq!(
        obj.len(),
        3,
        "expected 3 top-level siblings (law, peer_discovery, cron_sweeps), got {:?}",
        obj.keys().collect::<Vec<_>>()
    );
    assert!(
        obj.contains_key("cron_sweeps"),
        "cron_sweeps must be a top-level sibling"
    );

    // peer_discovery has exactly its own 2 children — cron_sweeps is NOT absorbed.
    let pd = obj
        .get("peer_discovery")
        .and_then(Value::as_object)
        .expect("peer_discovery is a map");
    assert_eq!(
        pd.len(),
        2,
        "peer_discovery must have exactly 2 children (pattern, zero_peers)"
    );
    assert!(
        !pd.contains_key("cron_sweeps"),
        "cron_sweeps must NOT be nested under peer_discovery"
    );

    // cron_sweeps keeps its own children.
    let cs = obj
        .get("cron_sweeps")
        .and_then(Value::as_object)
        .expect("cron_sweeps is a map");
    assert_eq!(cs.len(), 2, "cron_sweeps must have exactly 2 children");
    assert_eq!(
        cs.get("must_be").and_then(Value::as_str),
        Some("idempotent")
    );
}

/// Scalars, lists, and nested maps all project into the JSON data model.
#[test]
fn scalar_list_and_map_shapes() {
    let body = "name: sentinel\n\
                count: 3\n\
                enabled: true\n\
                forbid:\n  \
                - await\n  \
                - timeout\n\
                nested:\n  \
                a: 1\n  \
                b: 2\n";

    let v = config_value_from_yaml(body).expect("parse");
    let obj = v.as_object().unwrap();

    assert_eq!(obj.get("name").and_then(Value::as_str), Some("sentinel"));
    assert_eq!(obj.get("count").and_then(Value::as_i64), Some(3));
    assert_eq!(obj.get("enabled").and_then(Value::as_bool), Some(true));

    let forbid = obj
        .get("forbid")
        .and_then(Value::as_array)
        .expect("forbid is a list");
    assert_eq!(forbid.len(), 2);
    assert_eq!(forbid[0].as_str(), Some("await"));

    let nested = obj
        .get("nested")
        .and_then(Value::as_object)
        .expect("nested is a map");
    assert_eq!(nested.len(), 2);
}

/// A malformed YAML body errors rather than silently mis-parsing.
#[test]
fn malformed_body_errors() {
    // A tab in indentation is invalid YAML.
    let body = "a:\n\tb: 1\n";
    assert!(
        config_value_from_yaml(body).is_err(),
        "malformed YAML must error"
    );
}
