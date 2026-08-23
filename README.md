<p align="center">
  <img src="docs/logo/icon.svg" width="120" height="120" alt="claimdag">
</p>

# claimdag

**CAS claim and complete on a DAG. One writer. Unpacked Cap'n on disk.**

The host process is the sole mutator. Snapshot is `work.bin` (mmap).
This crate has no RPC surface.

```console
$ cargo add claimdag
$ cargo install --path crates/claimdag-cli
$ claimdag --dir /var/lib/seat upsert --summary "land the adapter"
$ claimdag --dir /var/lib/seat list
$ claimdag --dir /var/lib/seat list --json --all
$ claimdag --dir /var/lib/seat claim <id> --assignee <actor>
$ claimdag --dir /var/lib/seat complete <id>
$ claimdag --dir /var/lib/seat archive <id>
$ claimdag --dir /var/lib/seat link <parent> <child>
```

## Law

- One in-process graph. The host is the sole mutator.
- Snapshot is unpacked Cap'n `work.bin`. Hosts mmap it.
- Ids are 128-bit `WorkId`. Kind, status, and role are closed enums.
- Summary is the only open text field.
- `claim` is CAS on `gen` (omit `--gen` to ignore). Terminal is sticky.
- `archive` is a flag on a terminal node, not a new status and not a delete.
- Default `list` is live work only (`--terminal`, `--archived`, `--all`).
- `link` is a boolean hard dependency.

```toml
claimdag = { git = "https://github.com/HaoZeke/claimdag", tag = "v0.1.3" }
```

## WorkGraph pane

Textual tree. Mutations call `claimdag`. This process does not reimplement
claim, complete, unlink, or archive.

```console
$ pip install -e .
$ CLAIMDAG_DIR=/var/lib/seat claimdag-tui
$ claimdag-tui --dir /var/lib/seat --dump
```

Bindings: `c` claim, `d` complete, `a` archive, `h` show done, `A` show
archived, `u` unlink, `r` refresh. Theme is Tokyo Night Storm
(`#24283b` / `#c0caf5` / `#7aa2f7`).

Docs: [docs/orgmode/architecture.org](docs/orgmode/architecture.org).
Schema: [schema/claimdag.capnp](schema/claimdag.capnp).

License: Apache-2.0 OR MIT.
