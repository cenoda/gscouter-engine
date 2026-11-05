const { chromium } = require('playwright');
const fs = require('fs');
(async () => {
  const out = process.argv[2];
  const b = await chromium.launch();
  const p = await b.newPage({ locale: 'en-US', viewport: { width: 1400, height: 1000 }, userAgent: 'gscouter-engine/0.1 (+https://github.com/cenoda/gscouter-engine)' });
  await p.goto('https://wiki.hoyolab.com/pc/genshin/entry/' + (process.argv[3]||'5149') + '?lang=en-us', { waitUntil: 'networkidle', timeout: 90000 });
  await p.waitForTimeout(4000);
  let txt = '';
  const keys = await p.$$('.d-talent-keys-wrapper');
  for (let i = 0; i < keys.length; i++) {
    await keys[i].scrollIntoViewIfNeeded();
    await keys[i].click();
    await p.waitForTimeout(800);
    txt += '\n===== TALENT TAB ' + i + '\n' + (await p.$eval('.d-talent-content', e => e.innerText));
  }
  const btns = await p.$$('.m-d-btn-text');
  for (let i = 0; i < btns.length; i++) {
    const lab = await btns[i].innerText();
    await btns[i].scrollIntoViewIfNeeded();
    await btns[i].click();
    await p.waitForTimeout(600);
    const mod = await btns[i].evaluateHandle(e => e.closest('.entry-common-module') || e.parentElement.parentElement.parentElement.parentElement);
    txt += '\n===== ASCEND ' + lab + '\n' + (await mod.evaluate(e => e.innerText));
  }
  fs.writeFileSync(out + '/read.txt', txt);
  await b.close();
})();
