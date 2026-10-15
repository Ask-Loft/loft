# OpenSpec fixtures

Synthetic OpenSpec projects for `loft import openspec`, written for these tests. Each holds its
OpenSpec directory as `<project>/openspec/`, so the Product is named after `<project>`.

- `faithful`: main specs in OpenSpec's layout, flat and nested, a legacy `project.md`, a
  `config.yaml` with a `context`, an active change, and archived changes in both places
  OpenSpec keeps them. Its scenario bodies state obligations with SHALL, as OpenSpec's do.
- `quiet`: one main spec whose scenario bodies hold no modal word.
- `delta-only`: an active change and no main spec.
