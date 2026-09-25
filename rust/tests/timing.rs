//! Timings, which are left out of the default run since they assert nothing and exist to
//! give optimisation some numbers.
//!
//! ```
//! cargo test --test timing -- --ignored --nocapture
//! ```
//!
//! Run it before and after a change and put the numbers in the commit message. A change with
//! no numbers in the message is not an optimisation.

/// Builds a document of a size close to a real patch, with several directories, a batch of
/// files in each and a rule per directory.
///
/// # Arguments
///
/// * `dirs` - how many directories to declare
/// * `files_per_dir` - how many files to declare inside each directory
fn make_src(dirs: usize, files_per_dir: usize) -> String {
    let mut s = String::from("@ default\n");
    for d in 0..dirs {
        s.push_str(&format!("- dir{d:03}\n    @ delete\n    > protect\n"));
        for f in 0..files_per_dir {
            s.push_str(&format!("        | file{f:03}.jar\n"));
        }
    }
    s
}

/// Runs the closure three times and keeps the fastest run, since a single run is noisy in a
/// headless environment and both the fastest and the median beat it.
///
/// # Arguments
///
/// * `label` - the wording printed next to the timing
/// * `f` - the work to time, returning the size of its result
fn time(label: &str, f: impl Fn() -> usize) {
    let n = f();
    let mut best = std::time::Duration::MAX;
    for _ in 0..3 {
        let t = std::time::Instant::now();
        std::hint::black_box(f());
        best = best.min(t.elapsed());
    }
    println!("{label:<34} {n:>6} items   {best:?}");
}

#[test]
#[ignore = "for timing, not an assertion"]
fn timing() {
    let src = make_src(60, 40);
    let doc = chromatree::parse(&src).expect("the document should parse");
    println!(
        "document: {} lines, {} declarations",
        src.lines().count(),
        doc.declarations().len()
    );

    let decls = doc.declarations();
    let paths: Vec<Vec<String>> = decls
        .iter()
        .map(|x| x.path.iter().map(|s| (*s).to_string()).collect())
        .collect();

    time("declarations()", || doc.declarations().len());
    time("color_of, once per declaration", || {
        decls
            .iter()
            .filter(|x| doc.color_of(&x.path).is_some())
            .count()
    });
    time("explain, once per declaration", || {
        decls
            .iter()
            .filter(|x| doc.explain(&x.path).is_some())
            .count()
    });

    time("evaluate, with N queries", || {
        chromatree::evaluate(&doc, &paths).paths().len()
    });
    let c = chromatree::evaluate(&doc, &paths);
    time("roots_of", || c.roots_of("delete").len());
    time("counts", || c.counts().len());
    time("exceptions_within, ten calls", || {
        paths
            .iter()
            .take(10)
            .map(|p| {
                c.exceptions_within(&p.iter().map(|s| s.as_str()).collect::<Vec<_>>())
                    .len()
            })
            .sum()
    });
}
