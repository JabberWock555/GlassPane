# Contributing

Thanks for helping! glasspane is small on purpose. Contributions that keep
it **fast on old hardware** and **simple to run** are the most welcome.

## Good first contributions

- New tile kinds, such as scenes, scripts, covers or climate. Keep the
  allowlist in `src/config.rs` in sync, so the server only permits what the
  tile needs.
- Testing on other old tablets and browsers. Please report the device,
  Android and Chrome versions, and what broke.
- Translations of the few UI strings.
- Docs: setup notes for other servers (Pi, NAS, other phones).

## Before opening a pull request

1. `cargo fmt`, `cargo clippy --all-targets -- -D warnings` and `cargo test`
   all pass.
2. If you touched `web/`, run `npm run check` and `npm run build`, and
   **commit the updated `web/dist`**.
3. `node tests/e2e.mjs` passes against `mock_ha` + `demo.toml` (see
   [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)).
4. If the UI changed visibly, regenerate the screenshots.
5. **Comments explain the *why*.** Many choices here exist because of
   Chrome 57 or weak hardware. Say so in a comment where it isn't obvious.

## Ground rules

- **The tablet never gets the Home Assistant token**, and never gets
  entities that aren't on the dashboard.
- **Nothing in the frontend may need a browser newer than Chrome 57.** The
  build checks syntax and known APIs, but not CSS. Check CSS features on
  [caniuse.com](https://caniuse.com).
- **Prefer doing work once on the server** over doing it every frame on the
  tablet.
- **Don't commit personal data:** your own config, entity ids, IPs or
  photos. `glasspane.toml` is gitignored for this reason.

## Reporting bugs

Include:

- the tablet model, Android and Chrome versions
- how you run the server (Termux, Docker or native)
- the relevant lines from the server log, e.g.
  `tail -n 50 $PREFIX/var/log/sv/glasspane/current`
