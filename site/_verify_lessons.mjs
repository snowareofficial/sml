import fs from 'fs';
import { parseSafe } from '../js/sml.mjs';
fs.copyFileSync('./static/sml-verify.js', './_v.mjs');
const { VERIFIERS, CH_VERIFIERS } = await import('./_v.mjs');

const lessons = JSON.parse(fs.readFileSync('./data/sml-lessons.json', 'utf8'));
let allgood = true;
for (const [k, cfg] of Object.entries(lessons)) {
  const files = cfg.files || {};
  const r = parseSafe(cfg.main, { files });
  if (!r.ok) { console.log('PARSE FAIL', k, '->', r.error); allgood = false; continue; }
  const fn = VERIFIERS[k];
  const res = fn ? fn(r.value) : { ok: false, msg: 'no verifier' };
  if (!res.ok) allgood = false;
  console.log(k.padEnd(10), res.ok ? 'PASS' : 'FAIL', res.msg.slice(0, 50));
  if (!res.ok) console.log('   value=', JSON.stringify(r.value).slice(0, 220));
}
console.log(allgood ? '\nALL LESSONS PASS' : '\nSOME LESSONS FAIL');
fs.unlinkSync('./_v.mjs');
