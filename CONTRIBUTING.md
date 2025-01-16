# Contributing to Koto

Thank you for your interest in contributing to Koto!

## Reporting bugs

Please feel free to [open an issue][new-issue] if you find a problem in Koto.

## Improving the docs

As Koto is a new language with a goal of being easy to learn, one of the most important contributions you can make is to read the [language guide][guide] and [core library reference][core-lib]. If you find something confusing or incomplete, then its likely that others will to, and your suggestions for improvements will be invaluable.

The documentation is maintained in [this repo][docs]. To see how changes to the documentation look on the website, take a look at the [website's contributing guide][website-contributing].

## Working on issues

Please feel to take a look at the [open issues][open-issues] to see if there's something you'd like to work on. If you don't see anything that fits your interests then you're welcome to ask on [Discord][discord].

## Adding new libraries

The [`libs`][libs] directory includes several non-core libraries for Koto, and until Koto has a package management system, more could be added as long as they don't pull in large dependencies.

If you would like to add a new library, please make a proposal first in a new issue or discussion.

Libraries should include documentation for all new Koto functions in the [lib docs directory][lib-docs].

## Improving the website

The [Koto website][website] is in [this repo][website-repo], please refer to [its contributing guide][website-contributing].

## Improving performance

- Performance improvements for Koto are always welcome. There are a collection of benchmarks in the [koto/benches][benches] folder which can be run via `cargo bench`. The benchmarks aren't comprehensive, contributions are welcome! The benchmarks are configured in [crates/koto/benches][benches-runner].

[benches]: ./koto/benches/
[benches-runner]: ./crates/koto/benches/koto_benchmark.rs
[core-lib]: https://koto.dev/docs/next/core_lib
[discord]: https://discord.gg/JeV8RuK4CT
[docs]: ./crates/cli/docs
[libs]: ./libs/
[lib-docs]: ./crates/cli/docs/libs/
[new-issue]: https://github.com/koto-lang/koto/issues/new
[open-issues]: https://github.com/koto-lang/koto/issues/
[guide]: https://koto.dev/docs/next/language
[website]: https://koto.dev
[website-contributing]: https://github.com/koto-lang/koto.dev/tree/main/CONTRIBUTING.md
[website-repo]: https://github.com/koto-lang/koto.dev
