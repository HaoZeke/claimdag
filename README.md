# claimdag

CAS claim and complete on a DAG. The host process is the sole mutator.
Snapshot is `work.json`.

```console
$ cargo add claimdag
$ cargo install claimdag-cli
$ claimdag --help
$ claimdag --dir /var/lib/seat upsert --summary "land the adapter"
$ claimdag --dir /var/lib/seat list
$ claimdag --dir /var/lib/seat get <id>
$ claimdag --dir /var/lib/seat claim <id> --assignee <actor>
$ claimdag --dir /var/lib/seat complete <id>
$ claimdag --dir /var/lib/seat link <parent> <child>
$ claimdag --dir /var/lib/seat unlink <parent> <child>
$ claimdag --dir /var/lib/seat archive <id>
$ claimdag --dir /var/lib/seat list
$ claimdag --dir /var/lib/seat list --terminal
$ claimdag --dir /var/lib/seat list --all
```

Commands read and write `$dir/work.json`. Ids are 128-bit (`WorkId`, 32 hex
chars). Kind, status, and role are closed enums. Summary is the only open
text field. `claim` is compare-and-swap on `gen` (omit `--gen` to ignore).
Terminal status is sticky. `archive` is a soft hide on a terminal node, not
a delete and not a new status. Default `list` is live work only.
`link` is a boolean hard dependency; `unlink` drops it. `upsert` accepts
`--role` and `--parent`.

## WorkGraph pane

Textual tree of `$dir/work.json`. Mutations call `claimdag`; this process
does not reimplement claim, complete, or unlink.

```console
$ pip install -e .
$ CLAIMDAG_DIR=/var/lib/seat claimdag-tui
$ claimdag-tui --dir /var/lib/seat --dump
```

Directory is `$CLAIMDAG_DIR`, else `$XDG_RUNTIME_DIR/claimdag`. Bindings: `c`
claim, `d` complete, `a` archive, `h` show done, `A` show archived, `u`
unlink, `r` refresh. Default tree hides terminal and archived nodes.
Assignee is `$CLAIMDAG_ACTOR`
(32 hex) or a hash of `user@host`. Theme is Tokyo Night Storm
(`#24283b` / `#c0caf5` / `#7aa2f7`).

```toml
claimdag = { git = "https://github.com/HaoZeke/claimdag", tag = "v0.1.0" }
```

License: Apache-2.0 OR MIT.
