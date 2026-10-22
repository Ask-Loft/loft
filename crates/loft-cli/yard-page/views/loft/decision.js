// A `decision` view: the decision's title, label and state, its context and outcome, its
// options with the selected one marked, a table of its assessments, its consequences and its
// subjects.

import { h } from '../../lib/dom.js';
import { chip, named } from './index.js';

const RATINGS = {
  supports: 'success',
  neutral: 'muted',
  conflicts: 'danger',
};

const STATES = {
  accepted: 'success',
  proposed: 'info',
  rejected: 'danger',
  retired: 'muted',
  superseded: 'muted',
};

/** @param {any} report */
export function decisionView(report) {
  const decision = report.decision ?? {};
  const state = decision.state ?? {};
  const options = decision.options ?? [];
  const drivers = decision.drivers ?? [];
  const rating = new Map(
    (decision.assessments ?? []).map((assessment) => [`${assessment.option} ${assessment.driver}`, assessment]),
  );
  return [
    h(
      'header',
      { class: 'decision-view-head' },
      h('h5', {}, decision.label ? h('span', { class: 'label' }, decision.label) : null, ' ', decision.title),
      chip(state.kind ?? 'unknown', STATES[state.kind] ?? 'muted'),
      decision.label ? null : h('span', { class: 'unpublished' }, 'Not published yet: it has no label.'),
    ),
    decision.context ? h('p', { class: 'context' }, decision.context) : null,
    state.outcome ? h('p', { class: 'outcome' }, h('strong', {}, 'Outcome: '), state.outcome) : null,
    state.rationale ? h('p', { class: 'rationale' }, h('strong', {}, 'Rationale: '), state.rationale) : null,
    h(
      'ul',
      { class: 'decision-options' },
      options.map((option) =>
        h(
          'li',
          { class: ['decision-option', option.selected ? 'is-selected' : null] },
          h('strong', {}, option.title),
          option.selected ? chip('selected', 'success') : null,
          option.description ? h('p', {}, option.description) : null,
        ),
      ),
    ),
    drivers.length && options.length
      ? h(
          'div',
          { class: 'table-wrap' },
          h(
            'table',
            { class: 'assessments' },
            h(
              'thead',
              {},
              h(
                'tr',
                {},
                h('th', { scope: 'col' }, 'Option'),
                drivers.map((driver) =>
                  h(
                    'th',
                    { scope: 'col', title: driver.statement ?? '' },
                    driver.label ?? (driver.node ? named(driver.node) : driver.kind),
                  ),
                ),
              ),
            ),
            h(
              'tbody',
              {},
              options.map((option) =>
                h(
                  'tr',
                  {},
                  h('th', { scope: 'row' }, option.title),
                  drivers.map((driver) => {
                    const cell = rating.get(`${option.key} ${driver.key}`);
                    return h(
                      'td',
                      {},
                      cell ? chip(cell.rating, RATINGS[cell.rating] ?? 'muted') : '',
                      cell?.rationale ? h('p', { class: 'rationale' }, cell.rationale) : null,
                    );
                  }),
                ),
              ),
            ),
          ),
        )
      : null,
    drivers.length
      ? h(
          'details',
          { class: 'fold' },
          h('summary', {}, `${drivers.length} drivers`),
          h('ul', {}, drivers.map((driver) => h('li', {}, h('strong', {}, driver.label ?? driver.kind), ' ', driver.statement ?? (driver.node ? named(driver.node) : '')))),
        )
      : null,
    (decision.consequences ?? []).length
      ? h(
          'ul',
          { class: 'consequences' },
          decision.consequences.map((consequence) => h('li', {}, chip(consequence.kind, 'muted'), ' ', consequence.statement)),
        )
      : null,
    (decision.subjects ?? []).length
      ? h(
          'ul',
          { class: 'subjects' },
          decision.subjects.map((subject) =>
            h('li', {}, chip(subject.role, 'muted'), ' ', subject.node ? named(subject.node) : subject.former ?? ''),
          ),
        )
      : null,
  ];
}
