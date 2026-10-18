# Writing rules

Use this workflow for writing or rewording a node: how a product is broken down, which
statement form to use, how to word Requirements and their criteria, and where text that is not
an obligation belongs. It owns the craft. The fields and files belong to `format.md`; what each
lint finding means belongs to `lint-findings.md`. Never restate or override them here.

Lint checks some of these rules, and the rule ids are given where it does. Most of the craft is
judgement that lint cannot make.

## Contents

- Break a product down
- Choose a statement form
- Name the system
- Name each concept by one term
- Write criteria that are atomic and complete
- Give each rule of a catalogue its own Requirement
- Promote an oversized Requirement to a Feature
- Keep obligations out of bodies
- Make a Quality Attribute measurable
- Write Constraints
- Record decisions
- State a shared rule once, with a tag
- Cite a label with a `mentions` edge

## Break a product down

The chain runs Product → Feature Group → Feature → Requirement → criterion.

- A **Product** is what a user or a buyer would name: an application, a service, a library.
- A **Feature Group** gathers Features a reader would look for together.
- A **Feature** is one capability, small enough to build and test as a unit. Its name says what
  the user can do, such as "Add a book".
- A **Requirement** is one obligation of the Feature, stated in one of the five forms.
- A **criterion** is one checkable outcome of its Requirement.

Components are the parts that implement the product. A Feature `realizes` the Components that
implement it (`SH-001` reports a Feature that realizes none), and each leaf Component is
realized by at least one Feature (`SH-002`). A system outside the product that it depends on,
such as a payment provider, is a root Component with `boundary: external`.

## Choose a statement form

| The obligation holds | Form | Example |
|---|---|---|
| always | `ubiquitous` | The shelf shall sort books by title. |
| when an event happens | `event_driven` | When the reader adds a book, the shelf shall hold it. |
| while a state lasts | `state_driven` | While the device is offline, the shelf shall keep new books. |
| if something goes wrong | `unwanted` | If the title is empty, then the shelf shall refuse the book. |
| where a feature or option is present | `optional` | Where sync is on, the shelf shall copy books to the server. |

One statement states one obligation. A second condition, such as a refusal, is its own
Requirement in its own form, not a clause of the first. Write the statement as slots, never as
`text` (`V2-002`).

## Name the system

The `system` slot names who holds the obligation.

- A common noun takes "the": "the shelf", "The writer".
- A proper name stands bare: "Loft shall", "When …, Reading List shall". Never invent a head noun
  for it ("the Loft service"). List the project's proper names in `spec/lint.yaml` under
  `V2-008`'s `properNames`, and lint reports an article before one of them.
- Capitalise the system at the start of a `ubiquitous` statement; after a condition it follows a
  comma and keeps its own case.

## Name each concept by one term

A specification that calls one thing by two names reads as two things. Read `spec/terms.yaml`
before you write prose, if the project has one, and write each term's `name`, never one of its
`aliases` (`LQ-010` reports an alias).

Once a change to the prose is written, run `loft terms --format json`. It lists groups of names
that may denote one concept, such as "audit entry" beside "audit row", and the subjects of each
Product's statements. A group is a question, not a fault: put each group to the user, with the
nodes that use each name, and ask whether the names mean one concept.

- **One concept:** record the name the user chooses as a term, with a definition, and the other
  names as its `aliases`; then reword the prose that uses an alias.
- **Separate concepts:** record each as a term with its own definition, so that the difference is
  written down.

Either way the group is decided and `loft terms` stops listing it. Add no term and no alias the
user has not agreed: the list records the project's vocabulary, not your guess at it.

## Write criteria that are atomic and complete

Each criterion states one verifiable outcome, in one sentence (`V2-005` reports a semicolon or a
second sentence). Split a criterion joined by "and" or ", then" when its parts can fail apart;
one enumerated shape, such as the list of fields a document holds, may stay one criterion.

Together, the criteria cover the whole Requirement. To check:

1. List the Requirement's conditions: its trigger or state, each actor, each numeric limit, each
   failure and refusal, the empty and error states, and any order of events.
2. Map each condition to the criterion that covers it.
3. Write a criterion for each condition left uncovered, and question each criterion that maps to
   no condition: it is usually scope that belongs elsewhere.

A criterion that only repeats its Requirement covers nothing. "The book is added" under "When
the reader adds a book, the shelf shall hold it" is an echo; "The book appears on the shelf" and
"A book already on the shelf is not added twice" are outcomes.

## Give each rule of a catalogue its own Requirement

When a Feature lists rules, checks, commands or error codes, each item is its own Requirement
with its own criteria: when it applies, when it does not, and its exemptions. Never fold a
catalogue into one Requirement with a criterion per item.

## Promote an oversized Requirement to a Feature

A Requirement that holds more than one concern, many criteria (`V2-006` reports more than 20) or
a long body is a Feature in disguise. Promote it: create a Feature, move the Requirement under
it, and recompose the concerns into a few Requirements, each with atomic criteria. Never split it
into sibling Requirements that repeat one condition. Agree the new outline with the user before
writing the criteria.

## Keep obligations out of bodies

A body holds rationale, context and examples: why the node exists and what helps a reader. An
obligation belongs in a statement, a criterion, a Constraint or a decision, where it can be
checked; `V2-007` reports "shall" and "must" in a body. Prose that needs a heading goes in a
Section.

When a criterion has no Requirement that states its obligation, widen the Requirement's
statement or move the criterion; never complete the body instead.

## Make a Quality Attribute measurable

A Quality Attribute's name states the target in one sentence, and its `qualityTarget` makes it
measurable: a metric, a band with its bounds, a unit, an aggregation, the scenario that is
measured and the environment it is measured in. "Fast" is not a target; "within 200 milliseconds
at the 95th percentile, adding one book to a shelf of 500" is.

## Write Constraints

A Constraint is a rule a Component, a Feature Group or a Product must keep whatever the Feature: a
limit, a technology choice, a boundary. It binds what it stands under: a rule for one Component
goes on the Component, one for a group of Features on the Feature Group, and one for the whole
product, such as the language all its code is written in, once on the Product. Never copy one
rule onto several nodes. Name it as one "shall" sentence. Bind it to a category with a
required value when the value matters on its own, such as a storage limit.

What the product leaves out is not a Constraint: "shall exclude waitlists from the first version"
binds nothing and goes stale with the next version. Nor is it a criterion: "the message contains
no QR code" only restates an exclusion. Record a choice to leave something out as a decision, and
state the edge of scope in the body of the Product or Feature it bounds.

## Record decisions

Record a choice that shaped the product as a decision: its context, the drivers that weigh, at
least two options, an assessment of each option against each driver, the selected option, the
outcome, and the consequences.

Create a decision in the state that is true. A choice still open is `proposed`, and needs no
options or assessments yet. A choice already made is `accepted`, and then holds at least two
options, a driver, an assessment for each option and driver, a selected option, an outcome, a
consequence, and an `outcome` subject naming the node that carries the decision out. Write an
accepted decision in one pass: name its options, drivers and subjects, as `the-loop.md` shows,
and `loft fmt` writes their keys.

A decision with no label in `spec/labels.yaml` has not been published: correct it in place. Once
it has a label, it is never edited into a different choice: a new decision supersedes it, and you
ask the user before writing one.

## State a shared rule once, with a tag

When the same rule binds several Features, state it once, as a Requirement whose system is "a
command tagged `reads-tree`" or the like, define the tag in `spec/tags.yaml`, and list the tag on
each Feature it binds. Never copy the rule into each Feature.

Name a tag as the text writes its term. A one-word tag, such as `offline`, matches that word
wherever prose uses it; a tag of several words joined by hyphens, such as `shared-rule`, matches
only text that joins them by hyphens too, so ordinary prose with the same words is not a use of
the tag (`LQ-009`).

## Cite a label with a `mentions` edge

Prose may cite a published label, such as `RQ-12`. Add a `mentions` edge from the node to the
labelled node's key, so that the citation follows the node when labels change (`V2-003`). Prefer
the node's name in prose that does not need the label.
