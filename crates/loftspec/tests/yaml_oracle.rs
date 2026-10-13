//! Every document the reader accepts reads the same in a conforming YAML parser (ADR-14).

use crate::oracle;

const ACCEPTED: &[&str] = &[
    "key: 0190f5f6-0000-7000-8000-000000000001\nrequirements:\n  - key: a\n    name: \"N\"\n  - key: b\n",
    "tags:\n- portable\n- git\nname: \"x\"\n",
    "a: plain\nb: \"quoted \\\"x\\\"\\n\\t\\u00E9\"\nc: |\n  line one\n  line two\n",
    "a: |-\n  text\n\n",
    "a: |\n  text\n\n\n",
    "a: |+\n  text\n\n\nb: x\n",
    "a: |\n  one  \n  \ttab\n    deeper\n\n  after blank\nb: x\n",
    "a: |2-\n    indented\n  plain\n",
    "a:\nb: x\n",
    "a: C#\nb: \"issue #4\"\nc: |\n  # Heading\n  text\n",
    "- key: a\n  sections:\n    - name: \"S\"\n      body: |\n        nested\n- key: b\n",
    "counters:\n  P: 1\n  FG: 12\nbindings:\n  - key: k\n    label: RQ-1\n    aliases:\n      - RQ-9\n",
    "a: \"\"\nb: |+\n\nc: \"\\u2028\\\\\"\n",
];

#[test]
fn every_accepted_document_reads_the_same_in_a_conforming_parser() {
    for text in ACCEPTED {
        oracle::agree(text);
    }
}
