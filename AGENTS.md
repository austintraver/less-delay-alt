# less-delay-alt

Austin's fork of [Naxdy/less-delay](https://github.com/Naxdy/less-delay), kept so the plugin can be rebuilt for new Smash Ultimate versions.

Commit directly to `master`; Austin does not want pull requests in this repository.

Build on hoenn with `cargo skyline build --release` (cargo-skyline 3.5.0). The NRO lands in `target/aarch64-skyline-switch/release/libless_delay.nro` and installs at `sd:/atmosphere/contents/01006A800016E000/romfs/skyline/plugins/`.

Before trusting a build on a new game version, extract that version's `main` from the update NSP with nstool and run `tools/nso_text.py` then `tools/scan_needles.py` on it; every pattern must match exactly once.
