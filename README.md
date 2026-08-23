# claimdag

CAS claim and complete on a DAG. The host process is the sole mutator.
Snapshot is `work.json`.

```console
$ cargo add claimdag
$ cargo install claimdag-cli
$ claimdag --dir /var/lib/seat upsert --summary "land the adapter"
$ claimdag --dir /var/lib/seat list
```

Ids are 128-bit (`WorkId`, 32 hex chars). Kind, status, and role are closed
enums. Summary is the only open text field. `claim` is compare-and-swap on
`gen` (`0` means ignore). Terminal status is sticky. `link` is a boolean
hard dependency; `unlink` drops it.

```toml
claimdag = { git = "https://github.com/HaoZeke/claimdag", tag = "v0.1.0" }
```

License: Apache-2.0 OR MIT.
