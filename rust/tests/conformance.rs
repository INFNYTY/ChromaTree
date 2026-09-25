//! Runs the shared corpus in `conformance/` at the repository root.
//!
//! This is what decides whether the implementation follows specification 0.3, since every
//! language runs the same cases and without them the implementations would drift apart.
//!
//! The corpus format is described in `docs/*/contract.md`. The other test files stay away
//! from IO while this one has to read files, so the exemption below is explicit and applies
//! only here, the constraint of a library free of IO being covered by `clippy.toml` and
//! `-D clippy::disallowed_methods` rather than by the test harness.
#![allow(clippy::disallowed_methods)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chromatree::{check, check_vocabulary, evaluate, parse, Diagnostic};
use serde::Deserialize;

mod common;
use common::{probe_paths, walk_model};

const CORPUS_FILES: [&str; 3] = ["eval.json", "diagnostics.json", "queries.json"];

#[derive(Deserialize)]
struct Corpus {
    spec: String,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    source_file: Option<String>,
    #[serde(default)]
    vocabulary: Vec<String>,
    #[serde(default)]
    eval: Vec<EvalAssert>,
    /// `None` means the case leaves the diagnostics unasserted while `Some([])` asserts that
    /// there are none at all, and the two have to be told apart since
    /// `examples/permissions` is deliberately redundant and reports one warning.
    #[serde(default)]
    errors: Option<Vec<DiagAssert>>,
    #[serde(default)]
    warnings: Option<Vec<DiagAssert>>,
    #[serde(default)]
    paths: Vec<Vec<String>>,
    #[serde(default)]
    roots_of: BTreeMap<String, Vec<Vec<String>>>,
    #[serde(default)]
    paths_with: BTreeMap<String, Vec<Vec<String>>>,
    #[serde(default)]
    counts: BTreeMap<String, usize>,
    #[serde(default)]
    exceptions_within: Vec<ExceptionsAssert>,
    #[serde(default)]
    unmatched: Option<Vec<Vec<String>>>,
    #[serde(default)]
    explain: Vec<ExplainAssert>,
}

#[derive(Deserialize)]
struct EvalAssert {
    path: Vec<String>,
    color: Option<String>,
}

#[derive(Deserialize)]
struct DiagAssert {
    code: String,
    line: u32,
    /// `None` leaves the related positions unasserted.
    #[serde(default)]
    related: Option<Vec<u32>>,
}

#[derive(Deserialize)]
struct ExceptionsAssert {
    root: Vec<String>,
    paths: Vec<Vec<String>>,
}

#[derive(Deserialize)]
struct ExplainAssert {
    path: Vec<String>,
    chain: Vec<Link>,
}

#[derive(Deserialize)]
struct Link {
    color: String,
    line: u32,
    via: String,
    /// `None` leaves the column unasserted.
    #[serde(default)]
    col: Option<u32>,
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn load_source(case: &Case) -> String {
    match (&case.source, &case.source_file) {
        (Some(s), None) => s.clone(),
        (None, Some(f)) => {
            let p = repo_root().join(f);
            std::fs::read_to_string(&p)
                .unwrap_or_else(|e| panic!("{}: cannot read source_file {}: {e}", case.name, f))
        }
        _ => panic!(
            "{}: exactly one of source and source_file is needed",
            case.name
        ),
    }
}

/// The comparable form of a diagnostic, holding the short code, the line and the related
/// lines, which is compared after sorting since the specification fixes no report order.
type DiagKey = (String, u32, Vec<u32>);

fn key(d: &Diagnostic) -> DiagKey {
    let mut rel: Vec<u32> = d.related.iter().map(|s| s.line).collect();
    rel.sort_unstable();
    (d.code().to_owned(), d.span.line, rel)
}

fn want_key(d: &DiagAssert) -> DiagKey {
    let mut rel = d.related.clone().unwrap_or_default();
    rel.sort_unstable();
    (d.code.clone(), d.line, rel)
}

/// The `related` lines are asserted only when the case writes them, so the comparison trims
/// them to what the case asks for.
///
/// # Arguments
///
/// * `got` - the key of the diagnostic produced
/// * `want` - the expectation of the case
fn matches(got: &DiagKey, want: &DiagAssert) -> bool {
    got.0 == want.code
        && got.1 == want.line
        && match &want.related {
            None => true,
            Some(r) => {
                let mut w = r.clone();
                w.sort_unstable();
                got.2 == w
            }
        }
}

fn diff<T: std::fmt::Debug>(what: &str, got: &[T], want: &[T]) -> String {
    format!("{what}:\n  got {got:?}\n  want {want:?}")
}

fn refs(p: &[String]) -> Vec<&str> {
    p.iter().map(|s| s.as_str()).collect()
}

/// Compares the diagnostics with what the case expects and returns a readable reason on
/// failure.
///
/// The severity comes straight from the implementation, where a failed parse makes every
/// diagnostic an error and a successful one leaves only UnknownColor an error, so the split
/// here is by `is_error` into two buckets, each compared with the expected `errors` and
/// `warnings`.
///
/// # Arguments
///
/// * `case` - the case holding the expectations
/// * `got` - the diagnostics the implementation produced
fn check_diags(case: &Case, got: &[Diagnostic]) -> Result<(), String> {
    if let Some(want) = &case.errors {
        let errors: Vec<DiagKey> = got.iter().filter(|d| d.is_error()).map(key).collect();
        if !same_set(&errors, want) {
            return Err(diff("the set of errors differs", &errors, &want_keys(want)));
        }
    }
    if let Some(want) = &case.warnings {
        let warnings: Vec<DiagKey> = got.iter().filter(|d| !d.is_error()).map(key).collect();
        if !same_set(&warnings, want) {
            return Err(diff(
                "the set of warnings differs",
                &warnings,
                &want_keys(want),
            ));
        }
    }
    Ok(())
}

fn want_keys(want: &[DiagAssert]) -> Vec<DiagKey> {
    let mut v: Vec<DiagKey> = want.iter().map(want_key).collect();
    v.sort();
    v
}

/// Compares the two multisets exactly, with equal counts and every expectation matched
/// against a diagnostic that no other expectation has claimed.
///
/// [`matches`] is used rather than comparing keys directly, since `related` is asserted only
/// when the case writes it.
///
/// # Arguments
///
/// * `got` - the keys of the diagnostics the implementation produced
/// * `want` - the expectations of the case
fn same_set(got: &[DiagKey], want: &[DiagAssert]) -> bool {
    if got.len() != want.len() {
        return false;
    }
    let mut used = vec![false; got.len()];
    for w in want {
        let Some(i) = (0..got.len()).find(|&i| !used[i] && matches(&got[i], w)) else {
            return false;
        };
        used[i] = true;
    }
    true
}

fn run_case(case: &Case) -> Result<(), String> {
    let src = load_source(case);
    let asserts_diags = case.errors.is_some() || case.warnings.is_some();
    let asserts_queries = !case.paths.is_empty()
        || !case.roots_of.is_empty()
        || !case.paths_with.is_empty()
        || !case.counts.is_empty()
        || !case.exceptions_within.is_empty()
        || case.unmatched.is_some()
        || !case.explain.is_empty();

    // 1) Parsing, where a failure makes parse the source of the diagnostics.
    let parsed = parse(&src);
    if let Err(diags) = &parsed {
        if asserts_queries || !case.eval.is_empty() {
            return Err(format!(
                "the source did not parse, so the assertions cannot go on: {:?}",
                diags.iter().map(key).collect::<Vec<_>>()
            ));
        }
    }

    // 2) Diagnostics, which come from check once the parse succeeds.
    let diags: Vec<Diagnostic> = match &parsed {
        Err(d) => d.clone(),
        Ok(doc) => {
            if case.vocabulary.is_empty() {
                check(doc)
            } else {
                let vocab: Vec<&str> = case.vocabulary.iter().map(|s| s.as_str()).collect();
                check_vocabulary(doc, &vocab)
            }
        }
    };
    if asserts_diags {
        check_diags(case, &diags)?;
    }

    let Ok(doc) = parsed else {
        return Ok(());
    };

    // 3) The color of each path.
    for a in &case.eval {
        let got = doc.color_of(&refs(&a.path));
        if got.map(str::to_owned) != a.color {
            return Err(format!(
                "color_of({:?}): got {:?}, want {:?}",
                a.path, got, a.color
            ));
        }
    }

    // 4) The queries that need a batch of paths to answer.
    if !asserts_queries {
        return Ok(());
    }
    let c = evaluate(&doc, &case.paths);

    for (color, want) in &case.roots_of {
        let mut got: Vec<Vec<String>> = c.roots_of(color).iter().map(|d| d.path.clone()).collect();
        got.sort();
        let mut w = want.clone();
        w.sort();
        if got != w {
            return Err(diff(&format!("roots_of({color})"), &got, &w));
        }
    }

    for (color, want) in &case.paths_with {
        let mut got = c.paths_with(color);
        got.sort();
        let mut w = want.clone();
        w.sort();
        if got != w {
            return Err(diff(&format!("paths_with({color})"), &got, &w));
        }
    }

    let counts = c.counts();
    for (color, want) in &case.counts {
        let got = counts.get(color.as_str()).copied().unwrap_or(0);
        if got != *want {
            return Err(format!("counts()[{color}]: got {got}, want {want}"));
        }
    }

    for a in &case.exceptions_within {
        let mut got: Vec<Vec<String>> = c
            .exceptions_within(&refs(&a.root))
            .iter()
            .map(|d| d.path.clone())
            .collect();
        got.sort();
        let mut w = a.paths.clone();
        w.sort();
        if got != w {
            return Err(diff(&format!("exceptions_within({:?})", a.root), &got, &w));
        }
    }

    if let Some(want) = &case.unmatched {
        let mut got: Vec<Vec<String>> = c.unmatched().iter().map(|d| d.path.clone()).collect();
        got.sort();
        let mut w = want.clone();
        w.sort();
        if got != w {
            return Err(diff("unmatched()", &got, &w));
        }
    }

    for a in &case.explain {
        let chain = c.explain(&refs(&a.path)).unwrap_or_default();
        // The column is asserted only when the case writes one, since it is easier for an
        // implementation to get wrong than the line, a BOM, CRLF and non ASCII text all
        // affecting it, so it is worth pinning separately without being required every time.
        let printable = |d: &chromatree::Decision<'_>| {
            (d.color.to_owned(), d.span.line, via_name(d.via).to_owned())
        };
        let got: Vec<_> = chain.iter().map(printable).collect();
        let w: Vec<_> = a
            .chain
            .iter()
            .map(|l| (l.color.clone(), l.line, l.via.clone()))
            .collect();
        if got != w {
            return Err(diff(&format!("explain({:?})", a.path), &got, &w));
        }
        for (d, want) in chain.iter().zip(a.chain.iter()) {
            if let Some(col) = want.col {
                if d.span.col != col {
                    return Err(format!(
                        "explain({:?}) column on line {}: got {}, want {}",
                        a.path, want.line, d.span.col, col
                    ));
                }
            }
        }
    }

    Ok(())
}

fn via_name(v: chromatree::Via) -> &'static str {
    match v {
        chromatree::Via::Subtree => "subtree",
        chromatree::Via::Explicit => "explicit",
        chromatree::Via::Inherited => "inherited",
    }
}

#[test]
fn the_shared_corpus_passes() {
    let dir = repo_root().join("conformance");
    let mut total = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for file in CORPUS_FILES {
        let path = dir.join(file);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let corpus: Corpus = serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("{file} is not a valid corpus: {e}"));

        assert_eq!(
            corpus.spec, "0.3",
            "{file} declares a spec version other than the one this implementation claims"
        );

        for case in &corpus.cases {
            total += 1;
            if let Err(why) = run_case(case) {
                failures.push(format!("[{}] {}\n  {why}", file, case.name));
            }
        }
    }

    println!("corpus: ran {total} cases");
    assert!(
        failures.is_empty(),
        "{} cases did not pass:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

/// The short code table is a cross language contract, so the enum of this implementation has
/// to agree with `conformance/codes.json` entry by entry.
#[test]
fn diagnostic_codes_match_the_shared_table() {
    let path = repo_root().join("conformance").join("codes.json");
    let text = std::fs::read_to_string(&path).expect("codes.json is readable");
    let table: Table = serde_json::from_str(&text).expect("codes.json is valid");

    // Every kind of this implementation has to be in the table with the same code, and every
    // entry of the table has to be claimed by one.
    let mine = chromatree::all_diagnostic_codes();
    assert_eq!(
        mine.len(),
        table.codes.len(),
        "the number of kinds differs from codes.json: {} here, {} in the table",
        mine.len(),
        table.codes.len()
    );
    for (kind, code) in &mine {
        match table.codes.get(*kind) {
            Some(want) => assert_eq!(code, want, "the short code of kind {kind} differs"),
            None => panic!("codes.json holds no kind {kind}"),
        }
    }
}

#[derive(Deserialize)]
struct Table {
    codes: BTreeMap<String, String>,
}

/// The mental model of §2.9 and evaluation give the same answer on every source in the
/// corpus.
///
/// These are two independent implementations, where `eval` decides reachability by
/// arithmetic on indices while `common::walk_model` walks down from each origin in earnest,
/// writing as it goes. Anywhere the two diverge, either `reaches` is wrong or the model
/// described in §2.9 is not exact.
///
/// This goes a step further than the corpus itself, which pins down what the result is while
/// this test pins down one way of understanding it.
#[test]
fn the_mental_model_holds_on_every_corpus_source() {
    let dir = repo_root().join("conformance");
    let mut checked = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for file in CORPUS_FILES {
        let text = std::fs::read_to_string(dir.join(file)).expect("the corpus is readable");
        let corpus: Corpus = serde_json::from_str(&text).expect("the corpus is valid");
        for case in &corpus.cases {
            let src = load_source(case);
            // A case that does not parse has no evaluation to talk about
            let Ok(doc) = parse(&src) else { continue };
            let probe = probe_paths(&doc);
            if probe.is_empty() {
                continue;
            }
            checked += 1;
            let walked = walk_model(&doc, &probe);
            for p in &probe {
                let refs: Vec<&str> = p.iter().map(|s| s.as_str()).collect();
                let want = walked.get(p).cloned().flatten();
                let got = doc.color_of(&refs);
                if got.map(str::to_owned) != want {
                    failures.push(format!(
                        "[{file}] {}
  path {p:?}: evaluation {got:?}, walk {want:?}",
                        case.name
                    ));
                }
            }
        }
    }

    println!("mental model: compared the two implementations on {checked} corpus sources");
    assert!(
        failures.is_empty(),
        "{} divergences:

{}",
        failures.len(),
        failures.join(
            "

"
        )
    );
}
