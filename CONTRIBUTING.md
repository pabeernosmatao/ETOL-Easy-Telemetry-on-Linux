# Contributing to ETOL

Thanks for taking the time to help. ETOL is a personal project, not a
commercial product. I can only develop and test it with the simulators,
hardware and Linux distributions installed on my own machine. Reports and
patches are welcome, but I cannot reproduce or fix what I cannot run.

Issues, questions and pull requests can be written in **English or Spanish**.

## Before opening an issue

- Read the [user guide](docs/GUIDE.md) and the [README](README.md).
- Search the existing issues to avoid duplicates.
- Update to the latest release and try again.
- Use one of the issue templates. Blank issues are disabled.

## Bug reports

Use the **Bug report** template and include:

- ETOL version (`etol --version` or the window title);
- Linux distribution and version;
- desktop and session (KDE, GNOME, niri, X11/Wayland);
- CPU/GPU and driver if relevant;
- simulator, Proton/runner and Bottles/Flatpak setup;
- hardware involved (HF8, wheelbase, pedals...);
- steps to reproduce, expected behaviour and actual behaviour;
- logs.

### Logs

Use the **Capture logs** button in ETOL or run:

```bash
./scripts/recoger-logs.sh
```

Attach the generated archive or paste the relevant lines. Logs may contain
paths and device names; remove API keys, passwords and personal data before
posting.

## Feature requests

Use the **Feature request** template and explain the use case. I can only
implement and test features for hardware, simulators and distributions I have
access to. A feature that depends on a specific wheelbase, pedal set or distro
may be accepted as a patch, but I cannot verify it.

## Pull requests

1. Fork the repository and create a branch from `main`.
2. Keep the change focused; do not mix refactors, formatting and features.
3. Run the tests:

   ```bash
   ./scripts/cargo.sh test
   ```

4. If you change Rust, UI, scripts or the payload, build and test:

   ```bash
   ./scripts/gui.sh build
   ```

   The build needs the local `.toolchain` and `node_modules`
   (`./scripts/instalar-toolchain.sh` and `npm ci`) and a sibling
   `SIMHUB_LINUX` tree for `scripts/preparar-payload.sh`. If you do not have
   `SIMHUB_LINUX`, set `ETOL_KEEP_PAYLOAD=1` to keep the committed payload.

5. Update translations in `ui/i18n.js` (Spanish, Catalan, English) and the
   guides in `docs/guia-*.html` when the change is user-facing.
6. Do not commit game files, installers, keys, tokens or personal data.
7. Open the PR and fill in the template. Explain what you tested and on which
   distribution and hardware.

I may close a PR if I cannot test it, if it is out of scope or if it conflicts
with the project's direction. It is not personal.

## Project layout

| Path | What it is |
|---|---|
| `src/` | Rust core library and the `etol` CLI |
| `src-tauri/` | Tauri GUI and the bundled `payload/` |
| `ui/` | HTML/CSS/JS frontend; translations in `ui/i18n.js` |
| `scripts/sistema/` | Bottles and prefix setup scripts |
| `scripts/juegos/` | Per-game scripts and bridges |
| `docs/guia-*.html` | User guide (Spanish, Catalan, English) |
| `docs/RELEASE-*.md` | Release notes |

## Style

- Rust 2021, 4 spaces. Keep the surrounding style; comments are mostly in
  Spanish.
- Keep the CLI and the GUI in sync: both share `src/`.
- Avoid new dependencies unless they are really needed.
- User-facing strings go through `ui/i18n.js`; add the three languages.
- Do not modify the original `SIMHUB_LINUX` tree. Payload changes belong in
  ETOL scripts or the payload copy.

## Behaviour

Be respectful. Harassment, discrimination, piracy and spam are not welcome. I
may close or lock issues and block accounts to keep the repository useful.

## License

ETOL is MIT-licensed. By contributing you agree that your changes are released
under the same license.
