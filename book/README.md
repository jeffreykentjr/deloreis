# The De Lore Is book

An [mdBook](https://rust-lang.github.io/mdBook/) view of the repo's canonical
docs. It is a **scaffold**: it does not fork the docs, it renders them.

## How it is wired

The docs of record live under `docs/` (and the generated reference under
`docs/reference-generated/`). mdBook expects its pages under `book/src/`, so each
page here is a **thin wrapper** that pulls the real file in with mdBook's include
directive:

```markdown
{{#include ../../docs/charter.md}}
```

The include path is relative to the wrapper file. This keeps a single source of
truth: edit the doc under `docs/`, and the book re-renders it. The wrappers are
flat (all directly under `book/src/`) so every include path is the same shape;
`SUMMARY.md` supplies the chapter hierarchy.

Chosen over symlinks because the include is explicit, git-portable (no symlink
support assumptions), and lets a wrapper add a title above the included body.

**Known cosmetic limit:** the source docs carry YAML `--- status / verdict ---`
frontmatter, which mdBook renders verbatim at the top of each page (it has no
frontmatter stripper). This is acceptable for a scaffold; a later preprocessor
can strip it.

## Building

```bash
cargo run -p xtask --locked -- book   # → out/book (gitignored)
```

The build is **not** part of the offline gate — it shells out to `mdbook`. If
`mdbook` is not installed, the xtask step warns and exits 0. CI installs `mdbook`
in the docs-oracle job and builds the book there. To build directly:

```bash
mdbook build book -d ../out/book
```
