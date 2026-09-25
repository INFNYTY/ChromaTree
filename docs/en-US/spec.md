# ChromaTree Specification

**English** | [简体中文](../zh-CN/spec.md)

> This is the language specification. For a walkthrough see [tutorial.md](tutorial.md);
> for "what an implementation must provide" see [contract.md](contract.md), and for design
> rationale see [design.md](design.md).

---

## 1. Overview

ChromaTree (abbreviated ctree) is a declarative tree-tinting language.

Its core idea is that nodes of a parent/child tree are tinted in declaration order, that
later rules override earlier ones, and that path types control how tinting is inherited and
propagated.

ctree targets scenarios including, but not limited to:

- bulk permission / protection rules with local overrides
- theme, style and state tinting of a UI component tree
- diff-style marking of a filesystem or resource tree
- any assignment of tree-shaped attributes that needs "default + exceptions + isolation"

ctree assumes the real tree may have more children than the document declares. It describes
*which nodes receive which color*, not *every node*. That makes it a natural fit for
open-world, dynamically growing tree structures.

No color is a reserved word, and a color is any identifier such as `protect`, `delete`,
`allow` or `red`. The language does not know that any color is special, since it only puts
colors onto the tree and the user assigns the meaning.

---

## 2. Core Concepts

### 2.1 A Node Is a Set

In ctree every node is a set. A path node represents a set that may contain child elements,
and an individual node represents a set that is no longer expanded, that is, a leaf.

A node appears in the document because it needs a color, and a node has children because
its color needs to propagate to them or to be overridden locally.

### 2.2 Path Types

A path type determines how a node treats color coming from its ancestors, and whether its
own color keeps propagating downward.

| Symbol | Name | Accepts ancestor tint | Propagates down |
|--------|------|-----------------------|-----------------|
| `-`  | recursive path | yes | yes |
| `\|` | stop path | yes | no |
| `~`  | isolated path | no | yes (the color it started itself) |

### 2.3 Tint Operations

A tint operation decides who receives which color.

| Symbol | Name | Effect |
|--------|------|--------|
| `@`  | subtree tint | tints the current node; the color propagates downward from that node |
| `>`  | explicit tint | tints the explicitly listed direct children; each color continues along its own path type |

If a `>` target is a `~` node, that target is ignored (§2.4: `~` accepts no tint from
ancestors, and `>` travels down that channel). This is not an error, only a redundant
declaration.

### 2.4 The Propagation Law

Color propagates downward through the tree, and each node alone decides by its path type
whether to accept it and whether to pass it on. A parent never decides for its children.

The two tables above are two faces of this one law:

| Path type | Accepts color from ancestors | Passes its own color down |
|---|---|---|
| `-` recursive | yes | yes |
| `\|` stop | yes | no (stops here) |
| `~` isolated | no | yes (the color it tinted itself with `@`) |

`~` does not mean "colorless". It accepts nothing from others and honors only its own `@`,
and once it does tint itself the law above applies as usual, with each child deciding for
itself; its `@` stops only at a deeper `~` or at a `|`.

`|` means "stop here" and not "block". It accepts color from its ancestors but does not pass
it on, so its children have no color, and this is the only way to express "the directory is
colored, its contents are not" (§3.5). The word "block" belongs to `~` alone.

`@` and `>` do not decide scope, because the path type does. `@` merely hands a color to the
current node and `>` merely hands one to its targets, while whether it is accepted and
whether it propagates is entirely up to each node's own type.

### 2.5 Order and Override

The document executes in ascending line order, and later writes override earlier ones.

This holds for colors and path types alike: a path tinted by several rules takes its color
from the last of them, and a path declared several times takes its type from the last
declaration (§2.8). There is no exception anywhere, so a path's final state is what the last
thing to write to it wrote.

A rule of thumb follows from this, and it can be implemented as it stands:

> For any path, the effective color is the one given by the highest-line-numbered rule that
> reaches it.

"Reaches" is decided structurally by the propagation law of §2.4 (it does not depend on
whether the real tree contains that node, see §2.6), while the line number is a plain
numeric comparison. Keeping the two separate makes every placement of a rule well-defined.

One corollary: whether a rule is written before or after a child's declaration affects its
line number, not whether it can take effect. Both rules below reach `a/b`, and `@ red` has
the larger line number and wins (the example in §4.4).

### 2.6 The Open-World Assumption

ctree assumes the real tree may contain children the document never declares.

Recursive propagation reaches undeclared descendants on its own, so there is no need to
enumerate them. Consequently, a node that the document declares but the real tree does not
contain is perfectly normal, and it is simply a rule that is not in effect right now.

### 2.7 The Document Is an Anonymous Root

The document is itself a node: an anonymous root representing the whole tree's namespace.

A document is not a forest, and several nodes written at the outermost indent are siblings,
all children of this root. Tint operations may be written at the outermost indent, where
they sit in the anonymous root's scope: a top-level `@ c` tints the entire document, and the
targets of a top-level `> c` are those top-level nodes.

The anonymous root has no name, so it cannot be written, and it has no path type, since
semantically it behaves as a recursive path. Addressing starts at the root, which is the
empty path.

### 2.8 A Path May Be Declared More Than Once

```ctree
- root
    ~ saves          ← first declaration
        @ keep
    > protect
        - saves      ← second declaration
```

`saves` is declared twice. This is not an error, and the two declarations are not two nodes
either: they are two declarations of the same path.

The rules in both declarations take part in evaluation, ordered by line number without
discrimination. Above, `@ keep` is on line 3 and the `>` on line 4, so `saves` ends up
`protect`, with the `@ keep` inside the `~` overridden.

The path type comes from the last declaration. Swap the two around and the `~` lands last,
making `saves` an isolated path, whereupon the target of that `>` resolves to an isolated
path and is ignored (§2.3).

This is the same thing as writing two rule blocks for one selector in CSS, where the later
definition wins. It is §2.5's law applied to a type, since a path type is written too, and it
too belongs to the last writer.

Writing the same path twice is usually a slip, so the checks report a warning
(`DuplicatePath`, see [contract.md](contract.md#5-diagnostics)). A warning does not block
parsing: the document evaluates as usual.

### 2.9 Mental Model: a Depth-First Walk in Line Order

The best way to read a ctree document is as an imperative depth-first traversal. Indentation
gives the tree and line numbers give the order, so read in line order the document is the
pre-order traversal of that declared tree, where you meet a node and then its rules and
children (§4.4). Each step does one thing: either it declares a path, writing down who owns
it and what path type it is, or it tints.

A tint step walks downward from its origin: it does not enter an isolated path, and on
meeting a stop path it records and goes no further. That is what depth first means, and it
is also the propagation law of §2.4. Later writes override earlier ones, without exception
(§2.5).

One thing needs saying plainly: the path type a tint uses is the one the whole document
settles on, not "what had been written at the moment it ran".

```ctree
- a              ← by this step, a is still a recursive path
    - b
@ red            ← so this rule writes red into a and a/b
~ a              ← but over the whole document, a ends up an isolated path
```

By evaluation, `a` and `a/b` have no color, since `@ red` cannot enter an isolated path.
(Temporal intuition would make both red.) For why this side was chosen, see
[design.md](design.md#66-why-evaluation-and-not-temporal-order).

---

## 3. Mathematical Basis (Set-Theoretic View)

### 3.1 The Tree as a Poset

Let the real tree be \( T = (V, E) \), where \( V \) is the set of nodes and \( E \) the
parent/child relation. Define the partial order \( \preceq \): \( u \preceq v \) if
\( u \) is an ancestor of \( v \). For a node \( v \), its subtree set is

\[
S_v = \{ u \in V \mid v \preceq u \}
\]

that is, \( v \) together with all its descendants.

### 3.2 Tinting as a Partial Function

Tinting is a partial function:

\[
C: V \rightharpoonup \mathrm{Colors}
\]

meaning a node may have a color, or may have none. "Partial" is what lets a node be
undefined, as with `|` and `~` in §2.4.

### 3.3 Path Types as Inheriting Strategies

Define the visible ancestors function \( \mathrm{visible}(v) \): walk upward from \( v \),
stopping at a `~`. Only colors in \( \mathrm{visible}(v) \) affect \( v \), which is the
"accepting" side.

On the "passing on" side, when a node \( v \) passes its color to its descendants,

- `-`: keeps propagating to every descendant in \( S_v \);
- `|`: passes to no descendant;
- `~`: same as `-` (it passes on the color it tinted itself with), but it is a boundary
  for everyone else's \( \mathrm{visible} \).

The function \( \mathrm{visible}(v) \) excludes \( v \) itself, since otherwise `route1` in
`~ route1` could not even be tinted by its own `@`.

### 3.4 Tint Rules as State Transitions

Let the tint rules, in ascending line order, be \( R = (r_1, r_2, \dots, r_n) \), applied to
a tree whose structure is fixed by the node declarations. Each rule updates the partial
function \( C \).

- `@ color` on node \( v \):

\[
C(v) := color
\]

then, per \( v \)'s type (§3.3), `color` is passed to those nodes in \( S_v \) that should
receive it:

\[
\forall u \in S_v,\ u \succ v,\
\text{if the path from } v \text{ to } u \text{ contains no } \sim
\text{ and every node along it keeps propagating},\
C(u) := color
\]

- `> color` in the scope of node \( v \), with target set
  \( T \subseteq \mathrm{children}(v) \):

\[
\forall t \in T,\ \text{if } t \text{ is not } \sim,\ C(t) := color
\]

then each \( t \) applies the propagation rule above according to its own type.

Rules execute in line order, and later assignments override earlier ones.

### 3.5 Expressive Power

For any finite tree \( T \) and any coloring \( C \), there exists a ctree document that
describes it:

- for each subtree that needs a default color, use `@` on its root;
- for each child that needs an exception, use `>` on its parent;
- for each node that needs "colored itself but colorless contents", give it path type `|`;
- for each subtree that needs isolation from its ancestors, give it path type `~`.

The `|` case is indispensable, since without it there is no way to express "this node has a
color, its children have none".

---

## 4. Syntax

### 4.1 Lexical Structure

There are three kinds of line:

```
line       := blank | comment | declaration
comment    := indent ( "#" | "//" ) any text
declaration := indent symbol space name
name       := everything after that space up to end of line, trimmed, kept verbatim
```

where symbol is one of the five: `-`, `|`, `~`, `>`, `@`.

Names are opaque and never escaped, so all five symbols and both comment markers may occur
inside a name, and `/` is just an ordinary character that requires no escape. A name may
therefore contain spaces. Color names and node names follow the same rule, since both are
that name, and both are case-sensitive because they are strings and behave by string
comparison.

Comments occupy a line alone, and there are no inline comments and no multi-line comments.
A line that is none of the three kinds, and does not begin with one of the five symbols, is
an error and is never silently ignored. (Silently ignoring it would turn a missing `-` into
a whole subtree quietly moving elsewhere, which is the one failure mode a declarative
language must not have.) Blank and comment lines may appear anywhere and take no part in
evaluation.

Line endings and encoding are an input-level convention every implementation must match:

- The line break is `\n`. CRLF (`\r\n`) is accepted as usual, and the trailing `\r`
  belongs to the line break and not to the name.
- A lone `\r` is not a line break (that is the long-extinct old-Mac format). Such a file is
  read as a single line, so its `@`s and `-`s all become part of a name. The spec makes no
  attempt to accommodate it, but records the conclusion here so that implementations do not
  each decide for themselves.
- A UTF-8 BOM (`U+FEFF`) at the start of the document is ignored. Notepad on Windows writes
  one by default, and without handling it the first line looks like "a line not beginning
  with a symbol", so the whole document is wrong from line 1.
- Documents are UTF-8, so names and color names may be non-ASCII.

### 4.2 Structure

A ctree document is a series of declaration lines, and indentation expresses the
parent/child relation.

```ctree
- path-name
    @ color
    > color
        | child1
        | child2
```

`-`, `|` and `~` each declare a node. Tint operations (`@`, `>`) must be written inside some
node's scope, that is, indented under a node, and the anonymous root (§2.7) is a node too,
so top-level tint operations are legal. `>` may carry a group of node declarations as its
targets. `|` is a stop path, so it may not carry child declarations nor a `>` rule, since
both would contradict its definition. `~` is an isolated path, and rules inside it take
effect independently of its ancestors, but `~` itself may be tinted with `@`, and that color
propagates downward as usual.

### 4.3 Indentation

Tabs are forbidden in the indentation of a declaration line. Blank and comment lines take no
part in structure, so a tab before one has nothing to do with indentation being unambiguous
and is not an error.

The base indent must be a power of two, so 2, 4 or 8, and one base indent is used throughout
a document. Every line's indent is a multiple of that base, and the multiple is the depth. A
child's indent must be one level deeper than its parent's, since deeper by more than one
level is an error and is almost certainly a typo.

### 4.4 Execution Order

Tint rules execute in line order (§2.5). Depth-first traversal is a consequence of the
indentation structure rather than a separate rule, since the order of a rule relative to its
children is simply their order by line number, and node declarations themselves take no part
in evaluation order but supply structure only (for which declaration owns the path type when
a path is declared more than once, see §2.8).

This decides the meaning of a rule written after a child:

```ctree
- a
    - b
        @ blue
    @ red
```

`@ red` is written after `@ blue`, so `a` and its whole subtree (including `b`) end up `red`.

The recommended style writes defaults before children (see the tutorial's style guide),
where the two readings coincide, but the specification is line order.

---

## 5. Symbol Table

| Symbol | Kind | Name | Meaning |
|--------|------|------|---------|
| `-`  | path | recursive path | accepts ancestor tint, propagates downward |
| `\|` | path | stop path | accepts ancestor tint, does not propagate, has no children |
| `~`  | path | isolated path | accepts no ancestor tint, starts its own; its own tint propagates downward |
| `@`  | tint | subtree tint | tints the current node; the color propagates downward from it |
| `>`  | tint | explicit tint | tints the explicitly listed direct children; each color continues along its own path type |

> Additional structure: `#` and `//` begin a comment occupying a whole line. They are
> lexical line markers and take no part in tint semantics, so inside a name they are
> ordinary characters.

---

## 6. Version

The current language version is 0.3. It is recorded in the `spec` field of every conformance
corpus file ([conformance/](../../conformance/)), and each implementation declares in its own
package which version it implements.

Documents carry no version number and the syntax has no version directive, so a document
does not declare which version it belongs to. The version is a property of "this body of
corpus plus this specification".
