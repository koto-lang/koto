[![Koto](assets/koto.svg)][koto]

---

[![Docs](https://img.shields.io/docsrs/koto)][rust-docs]
[![Crates.io](https://img.shields.io/crates/v/koto.svg)][crates]
[![CI](https://github.com/koto-lang/koto/workflows/CI/badge.svg)][ci]
[![Discord](https://img.shields.io/discord/894599423970136167?logo=discord)][discord]

---

Koto is a simple and expressive programming language, usable as an extension
language for [Rust][rust] applications, or as a standalone scripting language.

## Info

- [About Koto](crates/cli/docs/about.md)
- [Koto Language Guide](crates/cli/docs/language_guide.md)
- [CLI Docs](crates/cli/docs/cli.md)
- [Online Playground][playground]
- [Example Rust application with Koto bindings](crates/koto/examples/poetry/)

## Development

The top-level [justfile](./justfile) contains some useful commands for working
with the repo, for example `just checks` which runs all available checks and 
tests.

After installing [just][just], you can run `just setup` to install additional
dependencies for working with the `justfile` commands.

## MSRV

Koto is under active development, and tested against the latest stable release
of Rust.

[ci]: https://github.com/koto-lang/koto/actions
[discord]: https://discord.gg/JeV8RuK4CT
[core-lib]: https://koto.dev/docs/next/core
[crates]: https://crates.io/crates/koto
[just]: https://just.systems/man/en/
[playground]: https://koto.dev/play
[rust]: https://rust-lang.org
[rust-docs]: https://docs.rs/koto
[koto]: https://koto.dev
