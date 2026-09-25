# Design Rationale

**English** | [简体中文](../zh-CN/design.md)

> This document records why things are the way they are, especially the options that
> were considered and rejected. For the rules themselves see [spec.md](spec.md); for the
> implementation contract see [contract.md](contract.md).

---

## 1. The Library Touches No IO

Checking and evaluation are pure functions, and the real tree arrives as segment sequences
supplied by the caller, so the library reads no files and no environment.

The open world (spec §2.6) means that which colors a document describes and what the real
tree contains are two different things to begin with, and merging them into one API would
force the library to guess which tree the caller meant. Pure functions can also be
property-tested unconditionally, with no temporary directory, no environment variable and
no cleanup, which is this library's largest testing advantage over its siblings. On top of
that, checking and evaluation have to reach the same conclusions about one document, and
one document can be evaluated against ten different trees, which is only natural if
evaluation holds no tree.

This one can be enforced at compile time, though the means differ per implementation, and each
implementation's own API contract is where its mechanism is recorded. The rule itself is the
shared part, and it lives in [contract.md](contract.md).

---

## 2. Names Are Opaque

Node names and color names are everything on a declaration line after that space, up to end
of line, trimmed and kept verbatim. The library knows no separator and neither splits nor
joins.

The reasoning is that the language has five symbols, and they constitute the entire grammar.
Introducing `/` introduces a sixth, and even if it appears in only one place the semantic
surface changes: from then on a name has two levels, segment and path, so the relation
between `a/b` and `a` + `b` must be defined, escaping must be defined, and absolute paths
must be defined. None of that has anything to do with tinting a tree; it is just one user
who happens to use `/` for hierarchy.

The cost is that callers split and rejoin themselves, though callers dealing with file trees
do it once and then forget about it.

The direct consequence is that the document cannot name a grandchild in one rule, because
`- textures/foo.png` can only match a single real segment, and no segment of a `/`-split
path looks like that. Nesting is how you say it:

```ctree
- textures
    > delete
        | foo.png
```

This is deliberate: depth is expressed by indentation, not by a separator inside names.

---

## 3. The Vocabulary Is Not in the Library

The library knows no color is special. The vocabulary for `check_vocabulary` is passed in by
the caller, and `Stats::colors` merely lists the colors appearing in the document for the
caller's use.

The reasoning is that `protect`, `delete` and friends are the user's business vocabulary,
not the language's, and the spec states plainly that no color is a reserved word. Baking in
one set also bakes in their semantics, and then changing vocabularies means changing the
library.

A side benefit is that the same ctree can describe permissions, UI themes and resource
trees, because it knows not one business word.

---

## 4. Diagnostics Carry No Prose

A `Diagnostic` has only `kind`, `span`, `severity` and `related`. Wording is produced by the
caller in its own language, and the library additionally provides a stable `code()` for logs.

Once the library hardcodes wording, that string is branded into the artifact in the language
it was packaged in, and the user's UI language has nothing to do with the packager's.
Localization is also the caller's job, and exposing `kind` is what lets a caller hang a
`kind → wording` table off it, with the compiler checking that table for gaps.

The cost is that the caller must write that table. That is acceptable, since they were going
to localize anyway.

An implementation can turn "produces no natural language" from a discipline into an assertion,
by testing that whatever it renders for a diagnostic stays within plain ASCII and carries no
wording of its own.

---

## 5. Diagnostics Do Carry a `code()`

While producing no prose, the library still gives every `DiagnosticKind` a stable short code
like `CT0107`.

The reasoning is that logs and issue tracking need an identifier that does not change with
wording. A user describes the error on line 12 in one language, and a developer receives a
different sentence, whereas `CT0107` is the same on both sides. This does not violate the
previous rule, since it is not a sentence, it is a key.

The convention is that once released, a code never changes meaning, and new diagnostics get
new codes rather than reusing old ones.

---

## 6. Rejected Designs

### 6.1 A `sep` Parameter (Rejected)

The option considered was `evaluate(doc, tree, sep)`, letting the library join ancestor names
with `sep` and split again, so that `- textures/foo.png`, one rule naming a grandchild,
would work.

This was rejected because it dresses `/` up as a sixth symbol. The moment the library
understands that there is a separator inside a name, it has path semantics: what an empty
segment means, what two separators in a row mean, and what leading or trailing separators
mean all have to be defined, and none of it has anything to do with tinting.

What the design settled on instead is opaque names (§2), depth via indentation, and
splitting by the caller.

### 6.2 A Lenient Lexer That Skips Lines Without a Leading Symbol (Rejected)

The option considered was that, since indentation and newlines exist, the parser could ignore
any line not beginning with one of the five symbols. That gives maximum document freedom, and
an author could write prose in it.

This was rejected because its failure mode is silent. A missing `-`, with `- mods` typed as
`mods`, would not error, and that line together with the indented children under it would be
attached to the nearest preceding node. The author believes the structure is unchanged while
a whole subtree has quietly moved, with no indication at all.

The entire value of a declarative language is stating scope unambiguously, so it is better to
error than to guess. That is why a line that is none of blank, comment or declaration reports
`UnexpectedLine`.

### 6.3 Inline Comments (Rejected)

The option considered was `#` and `//` starting a comment anywhere on a line.

This was rejected because it directly conflicts with the name running to end of line
unescaped. In `- a#b`, is the `#` part of the name or the start of a comment? Both
resolutions need an extra rule, either requiring a preceding space or forbidding `#` in
names, whereas a comment occupying its own line costs nothing, conflicts with nothing, and
keeps "what is a line" a three-way choice.

### 6.4 A Document as a Forest (Rejected)

The option considered was allowing several roots at the outermost level.

This was rejected because addressing becomes ambiguous: from which root does a path start,
and do top-level nodes get names, or must the user designate a root? Making the document one
anonymous root makes all of that disappear. The root is the empty path, top-level nodes are
its children, and a top-level `@` becomes the useful meaning of tinting the whole thing.

### 6.5 Duplicate Declarations Warn Rather Than Error

Declaring the same path twice does not block parsing, and reports one warning instead. The
rules in both declarations take part in evaluation, ordered by line number, and the path type
comes from the last declaration.

The language already treats later rules overriding earlier ones as a fundamental law, so
there is no reason for the same path written twice to suddenly become a hard error. The
second declaration is also often deliberate, laying a base and then amending one spot, which
is the shape of two rule blocks for one selector in CSS. Calling it an error would force the
author to fold them into one passage, which reads worse.

The cost is that the path type now follows the later definition winning too, so §2.5's rule
that node declarations take no part in order no longer holds. This is the real price, because
by right structure should be order-independent, and now there is one exception.

It is still worth it, because without the exception two declarations with different types
would need a winner chosen some other way, and any such choice needs an ordering criterion.
Since one already exists, it is better to say plainly that the last declaration wins.

How it used to be: this was originally an error, on the grounds that the same path declared
twice is two nodes competing for one address. That reasoning holds, but it brought something
worse, since the evaluator honoured only the first same-named node, so the second
declaration's rules vanished silently. That is the failure mode this design consistently
refuses, so a warning is better than leaving a path to silent misplacement.

### 6.6 Why Evaluation and Not Temporal Order

The path type a rule uses is the one the whole document settles on, which is the last
declaration of that path, not what had been written at the moment it ran. This is what the
note in §2.9 records.

ctree is declarative, so the temporal reading would make a rule's effect depend on where in
the file it sits, forcing the reader to track what the path is at that moment, and the
mental model of §2.9, an imperative walk, exists to help a human read a document, not to
decide how it evaluates. The rule of thumb in §2.5 also stays a single sentence, since a
path's color comes from the highest-line-numbered rule that reaches it, and reaching is a
structural judgement, independent of how far execution has run. CSS is non-temporal in the
same way, with two rule blocks for one selector and the later one winning regardless of
where the element appears; §2.8 already follows CSS, so this follows it as well.

The cost is that the propagation test in an evaluator is a static predicate, and path types
are resolved once over the whole document. That cost buys something back, since the
implementation and the document are isomorphic, and the walk downward from its origin
described in §2.9 can be implemented directly in that shape.

The readings diverge only in one narrow place, when a path is declared twice with different
types and a rule sits in between. Two cases under `declaration/*` in the corpus pin it.

---

## 7. Relationship to Sibling Projects

This library depends on, and knows, no particular consumer. Consumer-specific concerns, such
as not accepting nodes without a color, belong in a validator on the consumer's side rather
than as a rule baked into the language that only holds for them.
