// Node smoke test for the px-napi native addon (build-the-binary-run-the-binary).
//
// This loads the REAL compiled `.node` addon (produced by `napi build`) and
// exercises every exposed entry point against real `.px` input, asserting the
// results are sane. Nothing here is mocked: `parse`/`evaluate`/`checkConstraints`
// run the canonical Rust `px-compiler` + `px-eval` through N-API.
//
// Run: `node test/smoke.mjs` (from crates/px-napi, after `napi build`).

import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

// The addon is loaded via the generated CommonJS loader (`index.js`), which
// picks the correct platform `.node` binary.
const require = createRequire(import.meta.url);
const addon = require('../index.js');

let failures = 0;
function check(name, fn) {
  try {
    fn();
    console.log(`  ok   ${name}`);
  } catch (err) {
    failures += 1;
    console.error(`  FAIL ${name}\n       ${err.message}`);
  }
}

console.log('px-napi smoke test — loading real .node addon and exercising it\n');

// 1. The addon exposes the four functions we declared with #[napi].
check('addon exports parse/evaluate/checkConstraints/pxAstVersion', () => {
  for (const fn of ['parse', 'evaluate', 'checkConstraints', 'pxAstVersion']) {
    assert.equal(typeof addon[fn], 'function', `missing export: ${fn}`);
  }
});

// 2. pxAstVersion returns a real semver-ish string from the Rust crate.
check('pxAstVersion returns a version string', () => {
  const v = addon.pxAstVersion();
  assert.equal(typeof v, 'string');
  assert.match(v, /^\d+\.\d+\.\d+/, `unexpected version: ${v}`);
});

// 3. parse() on a real .px source returns the canonical kind-tagged AST as JSON.
const PX = `
import core::memory as mem

entity Conversation:
  prefix: "conv"
  fields:
    id: string
    priority: int

constraint no_empty_response:
  require: len($response) > 0
  severity: error
  message: "Empty responses are never acceptable"

constraint bounded_priority:
  when: $priority > 0
  require: $priority <= 10
  severity: warning
  message: "priority out of range"
`;

check('parse() returns a sane canonical AST for a real .px string', () => {
  const json = addon.parse(PX);
  assert.equal(typeof json, 'string');
  const ast = JSON.parse(json);

  // Root shape: { statements: [...] }
  assert.ok(Array.isArray(ast.statements), 'ast.statements must be an array');
  // import + entity + 2 constraints = 4 statements.
  assert.equal(ast.statements.length, 4, 'expected 4 top-level statements');

  // Kind-tagged enum encoding (Statement { kind, value }).
  const kinds = ast.statements.map((s) => s.kind);
  assert.deepEqual(kinds, ['Import', 'Entity', 'Constraint', 'Constraint']);

  // Dig into the entity to prove it is the real parsed structure, not canned.
  const entity = ast.statements[1].value;
  assert.equal(entity.name.name, 'Conversation');
  assert.equal(entity.prefix.value, 'conv');
  assert.equal(entity.fields.length, 2);
  assert.equal(entity.fields[0].name.name, 'id');
  // Type is the kind-tagged TypeExpr (Base -> String).
  assert.equal(entity.fields[0].field_type.kind, 'Base');
  assert.equal(entity.fields[0].field_type.value, 'String');
});

// 4. parse() throws a real error on malformed .px (no canned success).
check('parse() throws on malformed .px', () => {
  assert.throws(
    () => addon.parse('entity :::: not valid'),
    /px parse error/,
    'malformed .px must throw',
  );
});

// 5. evaluate() runs the real v1 expression evaluator.
check('evaluate() computes a real expression result', () => {
  // Unambiguous arithmetic (no precedence assumptions): 10 + 20 == 30.
  const out = addon.evaluate('$x + $y', JSON.stringify({ x: 10, y: 20 }));
  assert.equal(JSON.parse(out), 30, '10 + 20 == 30');

  // Multiplication alone.
  const mul = addon.evaluate('$x * 3', JSON.stringify({ x: 10 }));
  assert.equal(JSON.parse(mul), 30, '10 * 3 == 30');

  const boolOut = addon.evaluate('$n > 3 && $n < 100', JSON.stringify({ n: 42 }));
  assert.equal(JSON.parse(boolOut), true);
});

// 6. checkConstraints() runs the real constraint evaluator over the parsed doc.
check('checkConstraints() evaluates every constraint against the vars', () => {
  // response empty -> no_empty_response VIOLATED (error);
  // priority = 5 (in range, guard true) -> bounded_priority SATISFIED.
  const vars = JSON.stringify({ response: '', priority: 5 });
  const outcomes = JSON.parse(addon.checkConstraints(PX, vars));
  assert.equal(outcomes.length, 2, 'two constraints checked');

  const byName = Object.fromEntries(outcomes.map((o) => [o.name, o]));
  assert.equal(byName.no_empty_response.status, 'violated');
  assert.equal(byName.no_empty_response.severity, 'error');
  assert.equal(byName.bounded_priority.status, 'satisfied');
});

check('checkConstraints() marks not_applicable when a when-guard is false', () => {
  // priority = 0 -> bounded_priority guard ($priority > 0) false -> not_applicable.
  const vars = JSON.stringify({ response: 'hello', priority: 0 });
  const outcomes = JSON.parse(addon.checkConstraints(PX, vars));
  const byName = Object.fromEntries(outcomes.map((o) => [o.name, o]));
  assert.equal(byName.no_empty_response.status, 'satisfied'); // "hello" is non-empty
  assert.equal(byName.bounded_priority.status, 'not_applicable');
});

console.log('');
if (failures > 0) {
  console.error(`px-napi smoke: ${failures} check(s) FAILED`);
  process.exit(1);
}
console.log('px-napi smoke: all checks passed ✔');
