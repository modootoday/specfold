// Layout audit for the built site. Usage:
//   NODE_PATH=<dir with puppeteer-core> node audit.mjs <base-url> <out-dir> [tag]
// Reports page overflow, elements escaping their parent, console errors,
// failed requests and mermaid / KaTeX / three.js render failures.
import { createRequire } from 'module';
const require = createRequire(process.env.NODE_PATH + '/');
const puppeteer = require('puppeteer-core');
const [base = 'http://127.0.0.1:18433', out = '/tmp/audit', tag = 'run'] = process.argv.slice(2);
const pages = ['', 'getting-started', 'concepts', 'editing', 'checks', 'attachments', 'agents', 'import', 'commands'];
const widths = (process.env.WIDTHS || '1440,1280,1024,768,390').split(',').map(Number);
const themes = ['dark', 'light'];
const fs = require('fs'); fs.mkdirSync(out, { recursive: true });
const browser = await puppeteer.launch({ executablePath: '/usr/bin/google-chrome', args: ['--no-sandbox', '--hide-scrollbars'] });
let total = 0;
for (const p of pages) for (const w of widths) for (const t of themes) {
  const page = await browser.newPage();
  const issues = [];
  page.on('console', m => { if (m.type() === 'error') issues.push('console: ' + m.text()); });
  page.on('pageerror', e => issues.push('pageerror: ' + e.message));
  page.on('requestfailed', r => issues.push('failed: ' + r.url()));
  page.on('response', r => { if (r.status() >= 400) issues.push(r.status() + ': ' + r.url()); });
  await page.setViewport({ width: w, height: w === 1280 ? 800 : 900 });
  await page.goto(`${base}/${p ? p + '/' : ''}?theme=${t}`, { waitUntil: 'networkidle0' });
  await page.evaluate(async () => { for (let y = 0; y < document.body.scrollHeight; y += 400) { scrollTo(0, y); await new Promise(r => setTimeout(r, 60)); } scrollTo(0, 0); });
  await new Promise(r => setTimeout(r, 1500));
  const found = await page.evaluate(() => {
    const r = [];
    const de = document.documentElement;
    if (de.scrollWidth > de.clientWidth + 1) r.push(`page overflow ${de.scrollWidth}>${de.clientWidth}`);
    const scrolls = el => { for (let a = el.parentElement; a; a = a.parentElement) { const o = getComputedStyle(a).overflowX; if (o === 'auto' || o === 'scroll' || o === 'hidden') return a; } return null; };
    const name = el => el.tagName.toLowerCase() + (el.className && typeof el.className === 'string' ? '.' + el.className.trim().split(/\s+/).slice(0, 3).join('.') : '') + ' "' + (el.textContent || '').trim().slice(0, 30) + '"';
    for (const el of document.querySelectorAll('a,button,code,pre,table,kbd,span,svg,img,canvas,input,li')) {
      const b = el.getBoundingClientRect(); if (!b.width || getComputedStyle(el).position === 'fixed') continue;
      if (el.closest('.sr-only,[aria-hidden=true]')) continue;
      if (b.right > de.clientWidth + 1 || b.left < -1) { if (!scrolls(el)) r.push('off-screen ' + name(el)); continue; }
      const par = el.parentElement; if (!par || getComputedStyle(el).display === 'inline' && el.tagName !== 'CODE') continue;
      const pb = par.getBoundingClientRect(); const sc = scrolls(el);
      if (sc && sc.contains(par) === false) continue;
      if ((b.right > pb.right + 1 || b.left < pb.left - 1) && !(sc && par.contains(sc) === false && sc !== par && false) && !(sc && sc !== document.body && par.contains(sc) === false && getComputedStyle(par).overflowX !== 'visible')) {
        if (sc && (sc === par || par.contains(sc) === false) && sc.scrollWidth > sc.clientWidth) continue;
        if (getComputedStyle(par).overflowX !== 'visible') continue;
        r.push(`escapes parent by ${Math.round(Math.max(b.right - pb.right, pb.left - b.left))}px ${name(el)}`);
      }
    }
    const mm = document.querySelectorAll('code.language-mermaid').length, ms = document.querySelectorAll('svg[id^=mmd]').length;
    if (mm > ms) r.push(`mermaid not rendered ${ms}/${mm}`);
    for (const el of document.querySelectorAll('a,button')) { if (el.scrollWidth > el.clientWidth + 1 && getComputedStyle(el).overflow !== 'visible' || el.scrollHeight > el.clientHeight + 2 && getComputedStyle(el).display.includes('flex')) r.push('content overflows control ' + name(el)); }
    for (const t of document.querySelectorAll('.prose p')) { if (/^\s*\|.*\|/.test(t.textContent)) r.push('unrendered table'); }
    for (const t of document.querySelectorAll('.prose p, .prose li')) { if (/(^|\s)(#{1,6} |```|\$\$)/.test(t.textContent)) r.push('raw markdown ' + t.textContent.slice(0, 30)); }
    if (document.querySelector('.katex-error')) r.push('katex error');
    if (document.querySelector('[data-hero]') && !document.querySelector('[data-hero] canvas, [data-hero] svg')) r.push('hero missing');
    return [...new Set(r)];
  });
  issues.push(...found);
  const file = `${out}/${tag}-${p || 'index'}-${w}-${t}.png`;
  await page.screenshot({ path: file, fullPage: w !== 1280 });
  if (issues.length) { total += issues.length; console.log(`## ${p || 'index'} ${w} ${t}\n  ` + issues.join('\n  ')); }
  await page.close();
}
await browser.close();
console.log(`TOTAL ${total}`);
process.exit(total ? 1 : 0);
