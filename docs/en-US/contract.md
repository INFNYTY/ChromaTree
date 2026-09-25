# The Implementation Contract

**English** | [简体中文](../zh-CN/contract.md)

> [spec.md](spec.md) describes the language, while this file describes an implementation.
> Any ctree implementation, in any language, that meets what follows implements spec 0.3.
>
> The contract says *what must be answerable*, not *with what types*, so actual signatures
> live with each implementation, in its own API contract. The list of implementations is in
> [README.md](../README.md).
>
> The evidence is the corpus in [conformance/](../../conformance/), and passing it is what
> counts.

---

## 1. Four Boundaries

1. An implementation does no IO. It does not read files, environment variables or the
   current directory, because checking and evaluation are pure functions and the real tree
   arrives as segment sequences from the caller. Whether this can be enforced at compile
   time, and by what means, is up to each implementation, and the reasoning is in
   [design.md](design.md).
2. An implementation knows no business vocabulary, so no color is special to it. Whenever
   it matters which colors are meaningful, the caller passes the set in.
3. An implementation speaks no natural language, since a diagnostic carries a kind plus a
   location and never prose, and the wording is produced by the caller in its own language
   and phrasing. Implementations additionally provide a stable short code (like `CT0107`)
   for logs and issue tracking, which is a key rather than a sentence.
4. Names are opaque, and no separator is known to an implementation. Paths go in as segment
   sequences, and they come out as segment sequences.

## 2. Names and Paths

The spec says a node name is "everything after that space up to end of line, trimmed, kept
verbatim" (spec §4.1), so a name may be any string, including one that contains `/`, spaces,
or the five symbols.

An implementation consequently never splits anything on its own, so if a caller wants `/` to
mean hierarchy, the caller splits the path first.

The empty segment sequence `[]` denotes the anonymous root (spec §2.7).

## 3. The Required Surface

| Operation | Meaning |
|---|---|
| parse | text → a document, or all diagnostics. Any error means no document is produced (no partial AST by error recovery). |
| check | document → diagnostics. Needs no real tree. |
| check with a vocabulary | as above, plus "every color must be in the given set". |
| single query | document + segment sequence → a color, or "declines to say". Needs no real tree. |
| batch evaluation | document + a set of paths → a result that can be queried repeatedly. |

The single query is the core, because under the open world (spec §2.6) a path's color
follows from the document's structure and rules regardless of whether the real tree
contains it. Everything else derives from it.

Batch evaluation must answer:

| Query | Meaning |
|---|---|
| `color_of(p)` | the color of any path |
| `paths_with(c)` | paths in the set whose color is `c` |
| `roots_of(c)` | paths whose color is `c` and whose parent's color is not `c`, that is, "which subtrees are wholly `c`" |
| `exceptions_within(root)` | paths in `root`'s subtree whose color differs from `root`'s, which is what pierces a region |
| `counts()` | how many paths in the set have each color |
| `unmatched()` | nodes the document declares that the set does not contain |
| `explain(p)` | every rule affecting this path, ascending by line number |

- A parent's color in `roots_of` is queried structurally and need not be in the set, since
  otherwise a caller supplying only leaf paths gets wrong results.
- `unmatched`'s criterion is "no path in the set equals it or lies under it", since
  otherwise a caller supplying only leaves is flooded with intermediate directories.
- The last entry of `explain` is the one in effect, and the source location it carries is
  the only source of "which line did this verdict come from".

Every "verdict" carries four things: the path, the color, the source location, and how it
came about, which is tinted directly by `@`, tinted directly by `>`, or inherited from an
ancestor. The last one lets a caller distinguish "the author named it" from "it was
inherited", and only the former counts as an operation.

## 4. The Precise Definition of Evaluation

A rule has an origin: for `@ c` in node A's scope the origin is A, and for `> c` whose
target is T the origin is T (and if T is an isolated path the whole rule is ignored,
spec §2.3).

Starting from origin O, color `c` reaches node V if and only if:

- `V` is `O`, or
- along the path from `O` to `V` every intermediate node is a recursive path, and `O` is
  not a stop path, and `V` is not an isolated path.

The color of path `P` is the color of the highest-line-numbered rule reaching `P`
(spec §2.5), and if no rule reaches it the implementation declines to say.

## 5. Diagnostics

### 5.1 The Checklist

Structural

| kind | severity | Triggered by |
|---|---|---|
| `TabIndent` | Error | a tab in the indentation |
| `IndentBase` | Error | base indent is not 2 / 4 / 8 |
| `IndentMixed` | Error | an indent that is not a multiple of the base |
| `IndentJump` | Error | a child more than one level deeper than its parent |
| `UnexpectedLine` | Error | a line that is none of blank / comment / declaration |
| `MissingSeparator` | Error | no space after the symbol |
| `EmptyName` | Error | a declaration with no name after the symbol |
| `StopHasChildren` | Error | a stop path carrying child declarations or a `>` rule |
| `EmptyTargets` | Error | a `>` rule with no targets at all |
| `RuleOutsideScope` | Error | a tint rule outside any node's scope |
| `EmptyDocument` | Warning | no node declarations at all |

Semantic (decidable from the document alone; no real tree needed)

| kind | severity | Triggered by |
|---|---|---|
| `DuplicatePath` | Warning | a path declared more than once. Not an error: the rules in both declarations take part in evaluation and the path type comes from the later one (spec §2.8) |
| `ExplicitTargetIsolated` | Warning | a `>` target resolves to an isolated path, so the target is ignored and the rule is redundant |
| `DeadRule` | Warning | a rule completely overridden by a higher-line-numbered rule (partial coverage does not count) |
| `ConflictingRule` | Warning | two `@` rules in the same scope giving different colors |
| `RedundantRule` | Warning | `@ c` where an ancestor already propagated `c` down with no isolated path in between |
| `UnknownColor` | Error | a color not in the caller-supplied vocabulary |

### 5.2 Short Codes

`conformance/codes.json` is the authoritative table of short codes, and the only one. The
full list of kinds is in this file, while the code values are in that JSON. They are kept
apart because the codes are read by machines and the list is read by people.

Once released, a code never changes meaning, and new diagnostics get new codes rather than
reusing old ones. Every implementation should have a test comparing its own enum or
constant table against that JSON.

### 5.3 What `DeadRule` Requires of an Implementation

Its notion of "override" must share the propagation computation with evaluation. Two
separate implementations will drift, and then the checker claims "this never takes effect"
while the evaluator happily applies it. "Judge each piece of semantics exactly once"
holds inside an implementation too.

## 6. What Is Not in the Contract

- The serialized shape is not part of the contract, and each implementation decides it.
  Cross-language document exchange is not needed today either, because consumers exchange
  the flattened operation sequence rather than the tinted tree.
- The layout of rendered source is not part of the contract either, since only idempotence
  is promised (render, parse, render again is byte-identical), not an indent width, and not
  comment preservation.
- Convenience APIs such as statistics and declaration listings are better to have than to
  lack, and it is no violation to leave them out.
- Idiomatic types per language are not part of the contract either. See the note at the
  top.

## 7. Conformance Testing

The corpus lives in [`conformance/`](../../conformance/) as a set of JSON files, and each
implementation writes a small harness that reads it, runs it and reports.

```jsonc
// One case may carry evaluation, diagnostic and query assertions at once; the three
// files are grouped by primary concern, not by format.
{
  "name": "tutorial/first-example",       // unique; names the case when it fails
  "note": "tutorial §1",                   // optional, for humans
  "source": "- root\n    @ default\n",     // or "source_file", relative to the repo root
  "vocabulary": ["allow", "deny"],         // optional: given, check-with-vocabulary runs
  "eval":  [{ "path": ["root","fileA"], "color": "special" },
            { "path": ["root","missing"], "color": null }],
  "errors":   [{ "code": "CT0105", "line": 2, "related": [3] }],
  "warnings": [],
  "paths": [["mods"], ["mods","cache"]],   // the path set for batch evaluation
  "roots_of": { "delete": [["mods"]] },
  "paths_with": { "special": [["root","fileA"]] },
  "counts": { "delete": 4 },
  "exceptions_within": [{ "root": ["mods"], "paths": [["mods","saves"]] }],
  "unmatched": [["root","nowhere"]],
  "explain": [{ "path": ["mods","saves"],
                "chain": [{ "color": "delete", "line": 2, "via": "inherited" }] }]
}
```

- Paths are segment arrays, not `"a/b"` strings, because names are opaque and may contain
  `/`, which makes the string form ambiguous.
- Diagnostics use exact-set semantics, so reporting one extra is a failure: the spec lists
  the checks, and an extra one is non-conformance rather than being "stricter". Omitting
  `errors` / `warnings` means "this case does not assert diagnostics", while writing `[]`
  means "assert there are none", and the two differ.
- Report order is not specified, so equal sets pass.
- `related` is asserted only when it is written, since most diagnostics have no related
  second location.
- The corpus has no version directories, because the version is in each file's `spec`
  field and older versions are preserved by git tags.

`source` and `source_file` are mutually exclusive, and the latter is relative to the
repository root, so example files in `examples/` need not be copied into the corpus.
