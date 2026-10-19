# Reviewing a specification

Use this workflow for a review of a specification's quality beyond what `loft check` reports:
whether each Requirement's criteria cover it, whether nodes contradict one another, and whether
the product is broken down well, for a whole document or for the change a branch makes. It owns
the scope of a review, the judgement, the disposition of each finding and the report. What each
finding means belongs to `lint-findings.md`; the craft a review measures against to
`writing-rules.md`. Never restate or override them here.

A review is read-only. Change nothing in `spec/` unless the user asks for fixes.

## Contents

- Set the scope
- Start from lint
- Check coverage
- Look for contradictions
- Check the breakdown and the decisions
- Dispose of each finding
- Report
- Fix a large review in waves

## Set the scope

A review of a whole document reads every node. A review of a change reads what the branch added,
changed, moved or removed, against everything those assertions are read with:

1. Name the base the change is reviewed against, such as `develop`, and say it in the report.
2. Run `loft review --base <base> --format json`. It groups the assertions under review into one
   unit for each scope, the Feature, Feature Group, Product or Component that holds them, and
   gives each unit the nodes above its scope with their Constraints and Quality Attributes, every
   Requirement of a Feature with its criteria, the Components it realizes, the decisions and the
   terms its assertions bear on, and `before`, the base's text, for each changed, moved or
   removed assertion. It judges nothing.
3. Read each page in turn (`--page`), and each changed assertion against its whole unit,
   unchanged nodes included: most contradictions lie between a new assertion and an old one.

`loft review --node <label>` narrows either kind of review to one node and what lies beneath it.

## Start from lint

Run `loft check --format json` and report its errors and findings first, grouped by rule. For a
change, run `loft check --baseline <base> --format json` instead, and report the findings it marks
`introduced` apart from the `preexisting` ones and the `resolved` ones it lists. Then judge what
lint cannot.

Say what each finding means for the text, and what fixes it; never only repeat its message.

## Check coverage

For each Requirement in scope, list its conditions: the trigger or state, each actor, each
numeric limit, each failure and refusal, the empty and error states, and any order of events.
Map each condition to the criterion that covers it. Report each condition no criterion covers,
and each criterion that maps to no condition. A criterion that repeats its Requirement is a
mention, not coverage, and two texts that use the same words do not, by that, cover each other.

## Look for contradictions

- **Direct:** two statements or criteria that cannot both hold.
- **By condition:** the same trigger with different responses.
- **Numeric:** limits that disagree; compare "within", "at most" and "less than" exactly.
- **Against an ancestor:** a node that breaks a Constraint of a Component or Feature Group above
  it. A Constraint's `conservationKind` names the kind of invariant it guards, a sum preserved, a
  cap or an operation's identity; look for that kind of violation among its neighbours.
- **Precedence:** two assertions that are each right, where both apply and nothing says which
  wins.
- **Against its own criteria:** a criterion that contradicts its Requirement.
- **Against a decision:** a node that contradicts an accepted decision's outcome, or adopts what
  an option the decision rejected describes.

## Check the breakdown and the decisions

- Each Feature holds one capability; a Feature that holds two is two Features.
- Each Requirement states one obligation; one with many criteria or a long body is a Feature in
  disguise.
- Each obligation sits in a statement, a criterion, a Constraint or a decision, not in a body.
- Each accepted decision is complete: its options, drivers, assessments, selected option,
  outcome, consequences and outcome subject.

## Dispose of each finding

Each finding ends in one of three dispositions:

- **Fix:** the smallest edit that removes it.
- **Keep:** the node stays as it is, with the reason the finding does not hold for it.
- **Suppress:** a lint finding whose rule does not apply to its node, suppressed for that rule at
  that node alone as `lint-findings.md` says. Propose each suppression in the final report, all of
  them together; never stop the review to ask for one.

Never weaken what a node means to satisfy a rule.

## Report

Order the findings by severity: contradictions, then gaps in coverage, then the breakdown, then
wording. Name each finding's node by its label, or its name where it has none yet, quote the text
that shows it, and give the smallest edit that removes it. Report only what the text shows; never
manufacture a finding to look thorough, and when nothing is wrong, say so.

- Record what the review was made with: the `loft` version, the commit, and the lint
  configuration, `spec/lint.yaml` as it stood. Never compare two reports made with different
  versions or configurations.
- Say what you did not examine. A pass that found nothing in part of the text is no proof that
  nothing is there, and no proof about the rest.

Offer fixes, and make them only when the user asks.

## Fix a large review in waves

A review with many fixes is made in waves, each a coherent set of them: one branch for each wave,
merged and published before the next starts. Before each wave, check its fixes against the latest
published state, since an earlier wave may have changed the nodes they touch.
