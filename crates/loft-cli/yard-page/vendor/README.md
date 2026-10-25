# Mermaid, as the Yard page draws its diagrams with it

- Release: Mermaid 11.17.2, MIT, Copyright (c) 2014 - 2022 Knut Sveidqvist (`mermaid.LICENSE`).
- Source: `package/dist/mermaid.min.js` of <https://registry.npmjs.org/mermaid/-/mermaid-11.17.2.tgz>.
- SHA-256 of that file before compression: `581ed7d74bd9048d0e3a91363927d72ef22942d7722546b27f7cc29e35390eb8`.
- Kept here compressed, as `mermaid.min.js.gz` (`gzip -9 -n`); `loft yard serve` serves it at
  `vendor/mermaid.min.js` with `Content-Encoding: gzip`, and the page loads it only when it
  draws a diagram.

`scripts/vendor-mermaid.sh` writes both files from the registry. To move to another release,
change the version there and here, run it, record the new digest here, and run the page's tests.

# Inter, as the Yard page sets its text in it

- Release: Inter, font version v20, as `@fontsource-variable/inter` 5.3.0 ships it, SIL Open Font License 1.1,
  Copyright 2016 The Inter Project Authors (`inter.LICENSE`).
- Source: `files/inter-latin-wght-normal.woff2` of
  <https://registry.npmjs.org/@fontsource-variable/inter/-/inter-5.3.0.tgz>, the Latin subset
  with every weight from 100 to 900.
- SHA-256: `3100e775e8616cd2611beecfa23a4263d7037586789b43f035236a2e6fbd4c62`.
- Kept here as `inter-latin.woff2`; `yard.css` names it in its `@font-face`, and the page falls
  back to the system's sans-serif for any character outside the subset.
