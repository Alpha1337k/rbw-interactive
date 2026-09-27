# rbw-interactive

A TUI frontend for [rbw](https://github.com/doy/rbw). It drives the `rbw` CLI, which talks to the running `rbw-agent`.

```sh
cargo run --release
```

## Keys

| Scope     | Keys                                                                                          |
|-----------|-----------------------------------------------------------------------------------------------|
| Global    | `q` quit, `<Tab>`/`<S-Tab>`/`gt`/`gT`/`1-3` switch tabs                                       |
| List      | `j/k`, `gg/G`, `<C-d>/<C-u>` move, `/` or `i` search, `<Esc>` clear, `l`/`<CR>` details       |
|           | `y` yank password, `u` yank user, `t` yank TOTP, `o` open in browser, `r` sync                |
| Search    | type a regex (smart case), `<C-n>/<C-p>` move, `<C-w>` delete word, `<C-u>` clear, `<Esc>` done |
| Details   | `j/k` select field, `y`/`<CR>` yank field, `s` reveal secrets, `p`/`u`/`t`/`o` as above, `h` back |
| Generator | `j/k` select setting or history, `h/l` adjust (`H/L` by 10), `r`/`n` regenerate, `y`/`<CR>` yank, `s` save to vault |
| Save form | `<Tab>`/`<S-Tab>` switch field, `<CR>` save, `<Esc>` cancel                               |
| Config    | `s` sync, `U` unlock, `L` lock, `l` log in again                                                                |

## Mouse

Click tabs, entries, fields, settings and history rows to select them. Clicking something already selected opens it (entries) or copies it (fields, history). Click `‹ ›` to change a setting, and scroll to move the selection. Hold `Shift` to select text in the terminal as usual.

To add a shortcut, add an `Action` variant and a row in `BINDINGS` (`src/keys.rs`), then handle it in `App::dispatch`.
