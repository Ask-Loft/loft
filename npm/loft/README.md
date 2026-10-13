# Loft

`loft` authors, checks, formats and publishes a specification in the Loft Specification Format, in
a git repository. This package carries the prebuilt `loft` binary for macOS, Linux and Windows on
x64 and arm64.

```sh
npm install --save-dev @ask-loft/loft
npx loft init
npx loft check
```

As git's merge driver for `spec/`: `npx loft merge-driver --install --command "npx --no-install loft"`.

Elsewhere, `cargo install loft-cli` builds it from source. Documentation, the Format and the
source: <https://github.com/Ask-Loft/loft>.
