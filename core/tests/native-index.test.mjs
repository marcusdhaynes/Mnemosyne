import test from 'node:test';
import assert from 'node:assert/strict';
import {normalizeIndexed, loadIndexed, withIndexed} from '../../web/native-index.js';

test('normalizeIndexed shapes takenAt and gps, and aliases Windows paths', () => {
  const raw = {
    'C:\\Photos\\IMG_0001.jpg': {takenAt: 1704067200000, gps: {lat: 38.7, lon: -9.14}},
    '/data/shot.jpg': {taken_at: '1704067200000'},
    '/data/place-only.jpg': {gps: {lat: 40.4, lon: -3.7}},
    '/data/empty.jpg': {},
    '/data/bad.jpg': {takenAt: 'nope', gps: {lat: 'x', lon: 1}},
  };
  const indexed = normalizeIndexed(raw);
  assert.deepEqual(indexed['C:\\Photos\\IMG_0001.jpg'], {
    takenAt: 1704067200000,
    gps: {lat: 38.7, lon: -9.14},
  });
  assert.deepEqual(indexed['C:/Photos/IMG_0001.jpg'], indexed['C:\\Photos\\IMG_0001.jpg']);
  assert.deepEqual(indexed['/data/shot.jpg'], {takenAt: 1704067200000});
  assert.deepEqual(indexed['/data/place-only.jpg'], {gps: {lat: 40.4, lon: -3.7}});
  assert.equal(indexed['/data/empty.jpg'], undefined);
  assert.equal(indexed['/data/bad.jpg'], undefined);
});

test('loadIndexed calls catalog_indexed and returns the Spin contract shape', async () => {
  const invoke = async (cmd, args) => {
    assert.equal(cmd, 'catalog_indexed');
    assert.equal(args.path, '/library');
    return {
      '/library/a.jpg': {takenAt: 1000, gps: {lat: 1, lon: 2}},
    };
  };
  const indexed = await loadIndexed('/library', invoke);
  assert.deepEqual(indexed['/library/a.jpg'], {takenAt: 1000, gps: {lat: 1, lon: 2}});
});

test('loadIndexed is empty when Tauri is unavailable', async () => {
  assert.deepEqual(await loadIndexed('/library'), {});
});

test('withIndexed drops into resolveMedia options as {indexed}', () => {
  const options = withIndexed({concurrency: 8}, {
    '/a.jpg': {takenAt: 5, gps: {lat: 1, lon: 2}},
  });
  assert.equal(options.concurrency, 8);
  assert.deepEqual(options.indexed['/a.jpg'], {takenAt: 5, gps: {lat: 1, lon: 2}});
});
