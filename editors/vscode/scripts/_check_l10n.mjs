// 运行时文案的 l10n 闸门：源码 `L("…")` ↔ `l10n/bundle.l10n.<locale>.json`。
//
// 除了「有没有漏翻」，还要判**翻译到底会不会被加载** —— 这条此前没人查，于是
// `package.json` 少一个 `l10n` 字段时：bundle 文件在、`_check_l10n` 全绿，
// 而扩展宿主根本不知道去哪找 bundle，`vscode.l10n.t()` 永远返回原文
// （英文界面上每个运行时提示都还是中文）。静默失效，只能靠断言拦。
//
// 用法：node scripts/_check_l10n.mjs   （失败即退出码 1）
import fs from 'node:fs';
import path from 'node:path';

const fails = [];
const fail = (msg) => fails.push(msg);

// —— ① 清单：`l10n` 必须是顶层字段、指向存在的目录、且没被打包忽略 ——
const pkg = JSON.parse(fs.readFileSync('package.json', 'utf8'));
if (!pkg.l10n) {
  fail('package.json 缺顶层 "l10n" 字段 —— VS Code 不知道去哪找 bundle，运行时翻译不会加载（英文界面仍是中文）');
}
const l10nDir = pkg.l10n || './l10n';
if (pkg.l10n && !fs.existsSync(pkg.l10n)) fail(`package.json 的 "l10n" 指向不存在的目录：${pkg.l10n}`);
if (fs.existsSync('.vscodeignore')) {
  const ignored = fs
    .readFileSync('.vscodeignore', 'utf8')
    .split(/\r?\n/)
    .map((s) => s.trim())
    .filter(Boolean);
  const hit = ignored.filter((l) => /^!?\.?\/?l10n(\/|$)/.test(l) || /^\*\*\/l10n/.test(l));
  if (hit.length) fail(`.vscodeignore 排除了 l10n（${hit.join(', ')}）—— 装进 .vsix 后翻译不会生效`);
}

// —— ② 取源码里所有 `L("…")` 字面量（运行时值，`\n` 已展开） ——
const files = ['src/extension.js', 'src/highlight.js', 'src/special-highlight.js'];
const re = /L\(\s*"((?:[^"\\]|\\.)*)"/g;
const srcKeys = new Set();
for (const f of files) {
  const s = fs.readFileSync(f, 'utf8');
  let m;
  while ((m = re.exec(s))) {
    try {
      srcKeys.add(new Function('return ' + JSON.stringify(m[1]))());
    } catch {
      // 字面量里有解不出来的转义：跳过（`_verify_ext` 的语法检查会先报）
    }
  }
}

// —— ③ 每个 locale 的 bundle 都必须覆盖全部源码文案 ——
const bundles = fs.existsSync(l10nDir)
  ? fs.readdirSync(l10nDir).filter((f) => /^bundle\.l10n\..+\.json$/.test(f))
  : [];
if (!bundles.length) fail(`${l10nDir} 里没有 bundle.l10n.<locale>.json —— 除源语言外没有任何译文`);

const unusedAll = new Set();
for (const file of bundles) {
  const locale = file.replace(/^bundle\.l10n\./, '').replace(/\.json$/, '');
  let bundle;
  try {
    bundle = JSON.parse(fs.readFileSync(path.join(l10nDir, file), 'utf8'));
  } catch (e) {
    fail(`${file} 不是合法 JSON：${e.message}`);
    continue;
  }
  const missing = [...srcKeys].filter((k) => !(k in bundle));
  const unused = Object.keys(bundle).filter((k) => !srcKeys.has(k));
  for (const k of unused) unusedAll.add(k);
  console.log(`[${locale}] source keys: ${srcKeys.size} / 已翻: ${srcKeys.size - missing.length}`);
  console.log(`MISSING in bundle: ${missing.length}`);
  for (const k of missing.slice(0, 30)) console.log('  MISSING:', JSON.stringify(k));
  if (missing.length) fail(`${file} 漏翻 ${missing.length} 条`);
  if (locale === 'en') {
    // README 引用的两行口径（en 为主译文）
    console.log('bundle keys matched:', srcKeys.size - missing.length);
    console.log('bundle keys not in source (unused):', unused.length);
    for (const k of unused.slice(0, 30)) console.log('  UNUSED:', JSON.stringify(k));
  }
}
if (unusedAll.size) fail(`bundle 里有 ${unusedAll.size} 条源码中不存在的键（改文案后没清理）`);

if (fails.length) {
  console.log('\nL10N CHECK FAILED:');
  for (const f of fails) console.log('  ✗ ' + f);
  process.exit(1);
}
console.log('\nL10N CHECK ALL PASS');
