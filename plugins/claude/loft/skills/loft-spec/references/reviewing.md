# Reviewing a specification

Use this workflow for a review of a specification's quality beyond what `loft check` reports:
whether each Requirement's criteria cover it, whether nodes contradict one another, and whether
the product is broken down well. It owns the judgement and the report. What each finding means
belongs to `lint-findings.md`; the craft a review measures against to `writing-rules.md`. Never
restate or override them here.

A review is read-only. Change nothing in `spec/` unless the user asks for fixes.

## Contents

- Start from lint
- Check coverage
- Look for contradictions
- Check the breakdown and the decisions
- Report

## Start from lint

Run `loft check --format json` and report its errors and findings first, grouped by rule. Then
judge what lint cannot.

## Check coverage

For each Requirement in scope, list its conditions: the trigger or state, each actor, each
numeric limit, each failure and refusal, the empty and error states, and any order of events.
Map each condition to the criterion that covers it. Report each condition no criterion covers,
and each criterion that maps to no condition. A criterion that repeats its Requirement is a
mention, not coverage.

## Look for contradictions

- **Direct:** two statements or criteria that cannot both hold.
- **By condition:** the same trigger with different responses.
- **Numeric:** limits that disagree; compare "within", "at most" and "less than" exactly.
- **Against an ancestor:** a node that breaks a Constraint of a Component or Feature Group above
  it.
- **Against its own criteria:** a criterion that contradicts its Requirement.
- **Against a decision:** a node that contradicts an accepted decision's outcome.

## Check the breakdown and the decisions

- Each Feature holds one capability; a Feature that holds two is two Features.
- Each Requirement states one obligation; one with many criteria or a long body is a Feature in
  disguise.
- Each obligation sits in a statement, a criterion, a Constraint or a decision, not in a body.
- Each accepted decision is complete: its options, drivers, assessments, selected option,
  outcome, consequences and outcome subject.

## Report

Order the findings by severity: contradictions, then gaps in coverage, then the breakdown, then
wording. Name each finding's node by its label, or its name where it has none yet, and quote the
text that shows it. Report only what the text shows; when nothing is wrong, say so. Offer fixes,
and make them only when the user asks.
