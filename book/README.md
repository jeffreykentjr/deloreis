# The De Lore Is book

The book presents the README, quickstart, Holmes casebook, guides, reference,
and internals in that order. Pages under `book/src/` include the repository's
source documents so the book and the files read on GitHub stay in sync.

Build a local preview with:

```sh
cargo run -p xtask --locked -- book
```

Open `out/book/index.html`. The build also renders the checked Holmes wiki and
checks local links and anchors. It requires `mdbook` on your `PATH`; the command
warns and skips the book if it is absent. CI installs it and saves a preview as
an artifact in the existing docs job.

The Pages workflow runs only after a non-prerelease GitHub release is published.
It builds the release tag and deploys to the `github-pages` environment. Pages
must be enabled for the repository before that deployment can succeed.
