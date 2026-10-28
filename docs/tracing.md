# Tracing tests to criteria

`loft.trace.yaml`, beside `spec/`, links each acceptance criterion, and each Constraint or
Quality Attribute whose `verificationFate` is `proved`, to the tests that verify it, by file and
test name, or says why no test can. Your agent keeps it as it writes the tests, and asks you to
accept a link only once it has read the test against the criterion as it now reads.

Each link holds the content hash its node had when a reviewer last accepted it, so a changed
criterion shows up as a stale link, and each test the digest of its tokens when it was last
recorded, so a rewritten test shows up too. `loft reconcile` locates each test in its file's
syntax tree for Rust, TypeScript, JavaScript, Python, Go and Java, and as a whole word elsewhere,
without running it, so it works for any language. It reports:

- nodes no link names
- links not yet reviewed, or gone stale
- tests changed since they were recorded
- recorded tests it can no longer locate, or cannot find in their files
- tests that a part of their file it cannot parse may cut short
- links to nodes that are gone

`loft reconcile --accept <label>` records a review, and CI runs
`loft reconcile --fail-on warning`. The trace file is not part of the specification: it enters
no root and no publish writes it.
