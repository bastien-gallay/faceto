# Contributing

Full guide:
[`CONTRIBUTING.md`](https://github.com/bastien-gallay/faceto/blob/main/CONTRIBUTING.md). Coding
standards — Tidy First, CUPID, YAGNI, TDD, commit style — are in
[`CODING_STANDARDS.md`](https://github.com/bastien-gallay/faceto/blob/main/CODING_STANDARDS.md).

The local gate mirrors CI:

```bash
just ci      # fmt → clippy → test → js → markdown → book → keyboard → deps → size → workflows → justfile
just docs    # build this book into docs/book
```

Two rules worth stating up front: **do not add a runtime dependency** (ask first, even for a
dev-dependency), and **separate structural "tidy" commits from behavioural ones**.
