# games

The user's own games, `lex00/*`, one folder per game. Text only. ROMs and anything lifted from them never enter this repo. Every commit passes the ROM-blocking hook and the scan, hosted CI repeats the scan, and anything touching a ROM runs on the runner on the user's machine.

## Working here

Agents work in a worktree of this repo and land through the gate. Nothing else goes on `main`. Full steps are in the tool repo: `gameprinter/docs/workers.md` and `gameprinter/docs/gate.md`.

```text
git worktree add ../wt-game -b task/my-fix
cd ../wt-game
eval "$($GP_HOME/bin/gp worker env NAME --shell)"
# change the game, then
just check
git diff > "$GP_WORKER_OUT/fix.patch"
gp model gate donkey-kong --patch "$GP_WORKER_OUT/fix.patch" --onto main -m "Donkey Kong: ..."
```

The gate scores the game's corpus before and after the patch, and lands it only if no clip got worse. Never push, never use `--no-verify`, and keep ROMs and anything made from them out of this repo.
