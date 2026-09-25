# ChromaTree Tutorial

**English** | [简体中文](../zh-CN/tutorial.md)

> For the language rules see [spec.md](spec.md). This file is about how to use it.

---

## 1. A First Example

```ctree
- root
    @ default
    > special
        | fileA
        | fileB
```

- `root` and its subtree are tinted `default`.
- `fileA` and `fileB` are explicitly tinted `special`, overriding the inherited
  `default`.
- Every other descendant of `root` is tinted `default`, even ones the document never
  declares.

Here `@` means that from here down everything is this color, and `>` means that these
children are the exceptions. One lays the base and the other names the exceptions, which
is the most common shape in ctree.

## 2. Handling Undeclared Children

Suppose the real tree also has `fileC` and `fileD` under `root`, not declared anywhere:

```ctree
- root
    @ default
    > special
        | fileA
```

- `root`'s `@ default` propagates to every descendant automatically, including `fileC`
  and `fileD`.
- `fileA` is explicitly tinted `special`.
- Undeclared nodes pick up the default color through recursive propagation.

This is the "open world": a document describes color regions, not an enumeration of nodes.
When the real tree grows something new, the rule already covers it and the document needs no
edit.

## 3. Protection and Exclusions

```ctree
- route0
    @ delete
    > protect
        | file0
        - route1
```

- `route0` is tinted `delete`, propagating to all its descendants.
- `file0` and `route1` are explicitly tinted `protect`, overriding `delete`.
- `route1` is a `-` path, so it goes on propagating `protect` downward, and its
  descendants are spared too.
- `file0` is a `|` path, so only it becomes `protect` and nothing else is affected,
  though it has no children anyway.

## 4. Isolation Roots

```ctree
- route0
    @ delete
    ~ route1
        @ protect
        > delete
            | file1
```

- `route0` is tinted `delete`, but `~ route1` does not accept that inheritance.
- `route1` starts its own tinting: the inner `@ protect` makes it and its subtree
  `protect`.
- `file1` is explicitly tinted `delete`, overriding the inherited `protect`.

The point of `~` is to reset a region, so that instead of overturning the outer color
rule by rule you declare that this region starts over.

## 5. Spare the Contents: `|` and "Stop Here"

A `-` path passes the color all the way down and a `~` path starts fresh, while `|`
means that propagation stops here:

```ctree
- mods
    @ archive
    > inspect
        | manifest
```

- `mods` and its whole subtree are `archive`.
- `manifest` is `inspect`.
- If `manifest` had contents of its own, they would have no color at all, because `|`
  does not pass anything down.

This is indispensable when a directory is colored but its contents must not be
implicated. Conversely, whenever you see `|`, remember that what is underneath it does
not inherit from it.

## 6. Writing the Same Path Twice

```ctree
- mods
    ~ saves
        @ keep
    > protect
        - saves
```

`mods/saves` is declared twice. That is not an error, and the two declarations are not
two nodes; they are two declarations of the same path:

- The rules in both declarations take part in evaluation, ordered by line number without
  discrimination. Above, `@ keep` is on line 3 and the `>` on line 4, so `saves` ends up
  `protect`, and the line inside the `~` is overridden.
- The path type comes from the later declaration. Swap the two and the `~` lands last,
  which makes `saves` an isolated path, whereupon the target of that `>` resolves to an
  isolated path and is ignored.

So later writes override earlier ones, for colors and for path types alike.

Still, writing the same path twice is usually a slip, and the checks report a warning. If
you really mean the later declaration to win, folding the two into one passage usually
reads better.

## 7. Recommended Patterns

- Default + exceptions: set the default with `@` and list the exceptions with `>`.
- Isolation blocks: use `~` for a self-contained subtree, which keeps outside colors out.
- Sequential override: put more specific rules later so that they win (§8.3).

---

## 8. Style Guide

The following are conventions, not grammar.

### 8.1 Indentation and Layout

- Use 2 - 4 spaces per level; tabs are forbidden.
- One tint operation per line.
- Indent a `>`'s child list under the operator.

### 8.2 Naming Colors

- Color names should carry business meaning: `allow`, `deny`, `highlight`, `dim`.
- Colors need no declaration, but list the ones you use in a comment at the top of the
  document.
- Both color names and node names are case-sensitive.

### 8.3 Order of Defaults and Exceptions

- Write the default (`@`) first, then the exceptions (`>`), then the children. This reads
  as laying the base and then patching it, and it keeps the rule that rules run in line
  order from biting.
- If you must write it the other way around, remember that later rules override earlier
  ones.

### 8.4 Avoid Redundant Rules

- Look for rules completely covered by an ancestor's propagation and delete them.
- A subtree whose color matches its parent's default needs no repeated `-` or `@`.
- Write a node out only when its color or isolation differs from its parent's.
- A `>` target written as a `~` node is also redundant, because `~` accepts no tint from
  ancestors and `>` works through that same channel, so such a rule never takes effect.

### 8.5 Use `~` for Complex Subtrees

- When a subtree needs fully independent tinting logic, use `~` instead of `-`.
- Re-declare every color it needs inside the isolation root; do not rely on inheritance.
- If the `~` node itself needs a color, use `@`; it propagates downward.

### 8.6 Comments

- A comment occupies a line on its own and begins with `#` or `//`; inline comments are
  not supported.
- Prefer a separate comment line next to a path node explaining its business meaning.

---

## 9. A Complete Example

```ctree
# a permission tinting example
- root
    @ default

    > protect
        | adminPanel
        - userData
            @ protect
            > delete
                | tempFile
            ~ systemCache
                @ delete
```

- `root` is tinted `default`, propagating to every descendant (declared or not).
- `adminPanel` and `userData` are explicitly tinted `protect`, overriding `default`.
- `userData` accepts `protect`, and being a `-` path it passes `protect` to its
  descendants.
- `tempFile` under `userData` is explicitly tinted `delete`.
- `systemCache` under `userData` is an isolation root: it accepts no inherited `protect`,
  and its `@ delete` tints it and its subtree `delete`.
- Every other child of `root` (declared or not) keeps inheriting `default`.

This structure is in fact redundant. `systemCache` does not need the isolation path,
since its inner `@ delete` would override the inherited `protect` anyway, and `userData`
does not need to tint itself, since `> protect` already did. It is written this way to
show ctree's character: you only write in order and only think about the current node and
its parent and children, and the result still comes out right.

---

## 10. Summary

ChromaTree (ctree) is a declarative language for tinting tree structures, with a few core
strengths:

- Local declaration: you think only about the current node and its direct children.
- Sequential override: rules execute in writing order, and later rules override earlier
  ones.
- Explicit scope: `~` isolation roots pin down where tinting boundaries lie, and `|` pins
  down where propagation stops.
- Open world: recursive propagation covers undeclared descendants on its own.
- Set-theoretic basis: the tree is a poset, nodes are sets, tinting is a partial
  function, and rules are state transitions.
- Minimal symbols: 3 path types and 2 tint operations.

Following this specification lets you express the most complex protection, exclusion and
override logic with the fewest rules.
