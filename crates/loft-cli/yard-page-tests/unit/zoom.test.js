// The zoom of an expanded diagram: fitting it, zooming about a point and panning.

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { LEAST, MOST, fit, pan, percent, zoomAt } from '../../yard-page/model/zoom.js';

const stage = { width: 1000, height: 600 };

/** Where the diagram's point `at` lies in the stage under `view`. */
function onStage(view, at) {
  return { x: view.x + at.x * view.k, y: view.y + at.y * view.k };
}

test('fitting shows the whole diagram in the stage, centred in it', () => {
  const view = fit(stage, { width: 2000, height: 400 });
  assert.ok(view.k > 0 && view.k < 1, `${view.k}`);
  const corner = onStage(view, { x: 2000, y: 400 });
  assert.ok(view.x >= 0 && view.y >= 0 && corner.x <= stage.width && corner.y <= stage.height, JSON.stringify(view));
  assert.ok(Math.abs(view.x - (stage.width - corner.x)) < 1e-9, 'centred across');
  assert.ok(Math.abs(view.y - (stage.height - corner.y)) < 1e-9, 'centred down');
});

test('fitting a small diagram scales it up to fill the stage', () => {
  const view = fit(stage, { width: 100, height: 50 });
  assert.ok(view.k > 1, `${view.k}`);
});

test('zooming keeps the point under the pointer over the same point of the diagram', () => {
  const view = { k: 1, x: 40, y: 30 };
  const pointer = { x: 300, y: 200 };
  const under = { x: (pointer.x - view.x) / view.k, y: (pointer.y - view.y) / view.k };
  for (const factor of [1.5, 0.5]) {
    const zoomed = zoomAt(view, factor, pointer);
    assert.equal(zoomed.k, factor);
    const there = onStage(zoomed, under);
    assert.ok(Math.abs(there.x - pointer.x) < 1e-9 && Math.abs(there.y - pointer.y) < 1e-9, JSON.stringify(there));
  }
});

test('the zoom stays between a tenth of the diagram’s own size and eight times it', () => {
  const point = { x: 0, y: 0 };
  assert.equal(zoomAt({ k: 1, x: 0, y: 0 }, 1000, point).k, MOST);
  assert.equal(zoomAt({ k: 1, x: 0, y: 0 }, 0.0001, point).k, LEAST);
  assert.equal(MOST, 8);
  assert.equal(LEAST, 0.1);
  assert.equal(fit(stage, { width: 1, height: 1 }).k, MOST);
  assert.equal(fit(stage, { width: 1e6, height: 1e6 }).k, LEAST);
});

test('panning moves the diagram across the stage at the same zoom', () => {
  assert.deepEqual(pan({ k: 2, x: 10, y: 20 }, -30, 45), { k: 2, x: -20, y: 65 });
});

test('the zoom reads as a whole percentage of the diagram’s own size', () => {
  assert.equal(percent({ k: 1.234, x: 0, y: 0 }), 123);
  assert.equal(percent({ k: 0.1, x: 0, y: 0 }), 10);
});
