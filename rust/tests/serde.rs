//! The serialization contract.
//!
//! The serialized form of `Document` is a public contract, since it may be carried inside
//! another file format. Field names and shape are therefore pinned by a snapshot, and a
//! rename shows up here at once.

#![cfg(feature = "serde")]

use chromatree::{parse, Document};

fn doc(src: &str) -> Document {
    parse(src).unwrap_or_else(|e| panic!("the document should parse: {e:?}"))
}

const SRC: &str = "\
- a
    @ red
    - b
";

#[test]
fn json_shape_is_pinned() {
    let d = doc(SRC);
    let json = serde_json::to_string_pretty(&d).expect("the serialization should succeed");
    assert_eq!(json, EXPECTED_JSON);
}

#[test]
fn round_trips_through_json() {
    let d = doc(SRC);
    let json = serde_json::to_string(&d).expect("the serialization should succeed");
    let back: Document = serde_json::from_str(&json).expect("the deserialization should succeed");
    assert_eq!(back, d);
    // The semantics survive the round trip as well.
    assert_eq!(back.color_of(&["a", "b"]), Some("red"));
    assert_eq!(back.color_of(&["a"]), Some("red"));
    assert_eq!(back.color_of(&["a", "c"]), Some("red"));
}

/// An empty document survives a round trip as well, where `root.name` is `None`.
#[test]
fn the_anonymous_root_survives_serde() {
    let d = Document::default();
    let json = serde_json::to_string(&d).expect("the serialization should succeed");
    let back: Document = serde_json::from_str(&json).expect("the deserialization should succeed");
    assert_eq!(back, d);
    assert_eq!(back.root.name(), None);
}

/// The golden sample of the serialized form, which breaks on a renamed field or variant and
/// on any change to the structure.
const EXPECTED_JSON: &str = r#"{
  "version": {
    "major": 0,
    "minor": 3
  },
  "root": {
    "name": null,
    "path_type": "Recursive",
    "span": {
      "line": 1,
      "col": 1,
      "start": 0,
      "end": 22
    },
    "items": [
      {
        "Child": {
          "name": "a",
          "path_type": "Recursive",
          "span": {
            "line": 1,
            "col": 1,
            "start": 0,
            "end": 3
          },
          "items": [
            {
              "Rule": {
                "op": "Subtree",
                "color": "red",
                "span": {
                  "line": 2,
                  "col": 5,
                  "start": 8,
                  "end": 13
                },
                "targets": []
              }
            },
            {
              "Child": {
                "name": "b",
                "path_type": "Recursive",
                "span": {
                  "line": 3,
                  "col": 5,
                  "start": 18,
                  "end": 21
                },
                "items": []
              }
            }
          ]
        }
      }
    ]
  }
}"#;
