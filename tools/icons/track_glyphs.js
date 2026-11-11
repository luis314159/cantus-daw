// Generates crates/cantus-ui/assets/icons/tracks.txt from the icon family's set 6, the track
// icons (`docs/round39/design/icons/glyphs-t-*.js`, `glyphs-t-families.js`,
// `set6-additions.md`): the picker's families in the owner's order, every icon's English and
// Spanish names and its search words, and each icon's drawing for every device size it is
// built for. The scripts are loaded in `picker.html`'s order, in one scope, as the sheet does.
//
// `node tools/icons/track_glyphs.js <icons design dir> > crates/cantus-ui/assets/icons/tracks.txt`
//
// The file is lines of tab-separated fields:
//   F <family id> <English title>             a family, in the picker's order
//   S <family id> <English title>             a sub-row of the family above
//   I <icon id>                               an icon of the row above (repeats allowed)
//   W <icon id> <en> <es> <aliases>           the icon's names and search words
//   L <family id> <icon id> <en> <es>         the icon's name in that family, when it differs
//   D <icon id> <size> <svg body>             a drawing on a size × size canvas
const fs = require('fs');
const path = require('path');
const dir = process.argv[2];
global.window = {};
// The sheets read the page address (`?mode=fit` shows the drawings before the pixel fixes).
global.location = { search: "" };
const read = f => fs.readFileSync(path.join(dir, f), 'utf8');
const v1 = ['glyphs-transport.js', 'glyphs-tools.js', 'glyphs-tools2.js', 'glyphs-tools3.js']
  .map(f => read(path.join('v1', f))).join('\n;\n');
const files = ['family.js', 'glyphs-n-transport.js', 'glyphs-n-tools.js', 'glyphs-n-tools2.js',
  'glyphs-n-windows.js', 'glyphs-m-mixer.js', 'glyphs-m-mixer2.js', 'glyphs-b-browser.js',
  'glyphs-b-browser2.js', 'glyphs-t-tracks.js', 'glyphs-t-tracks2.js', 'glyphs-t-tracks3.js',
  'glyphs-t-lib.js', 'glyphs-t-lib2.js', 'glyphs-t-lib3.js', 'glyphs-t-add-a.js',
  'glyphs-t-add-a-names.js', 'glyphs-t-add-b.js', 'glyphs-t-add-c.js', 'glyphs-t-final.js',
  'glyphs-t-guitars.js', 'glyphs-t-guitars-e.js', 'glyphs-t-guitars-a.js',
  'glyphs-t-guitars-mx.js', 'glyphs-t-guitars-pick.js', 'glyphs-t-guitars-px.js',
  'glyphs-t-guitars-px-fix.js', 'glyphs-t-guitars-px-fix2.js', 'glyphs-t-guitars-px-fix3.js',
  'glyphs-t-guitars-px-fix4.js', 'glyphs-t-guitars-px-fix5.js', 'glyphs-t-families.js'];
const src = files.map(read).join('\n;\n');
// Browser scripts share one global scope: run them as one, then read what they leave.
const out = new Function('window', v1 + '\n;window.G1 = G;\n;' + src +
  '\n;return { N, drawing, FAMILIES: window.TRACK_FAMILIES, LIB: window.TRACK_LIBRARY,' +
  ' WORDS: window.TRACK_WORDS, LABEL_IN: window.TRACK_LABEL_IN || {} };')(window);
// The project keeps an id in lower case, digits and hyphens (ADR 0113): the sheets'
// camelCase ids are written in kebab case ("trebleClef" → "treble-clef").
const kebab = id => id.replace(/([a-z0-9])([A-Z])/g, '$1-$2').toLowerCase();
const slug = s => s.toLowerCase().replace(/&/g, 'and').replace(/[^a-z0-9]+/g, '_').replace(/^_|_$/g, '');
const clean = s => String(s || '').replace(/[\t\r\n]+/g, ' ').trim();
const lines = [];
const missing = [];
for (const f of out.FAMILIES) {
  const fid = slug(f[0]);
  lines.push(['F', fid, f[0]].join('\t'));
  for (const id of f[1]) lines.push(['I', kebab(id)].join('\t'));
  for (const s of f[2] || []) {
    lines.push(['S', slug(s[0]), s[0]].join('\t'));
    for (const id of s[1]) lines.push(['I', kebab(id)].join('\t'));
  }
}
for (const id of out.LIB) {
  const w = out.WORDS[id];
  if (!w || !w[0] || !w[1]) missing.push('words:' + id);
  const [en, es, more] = w || [id, id, ''];
  lines.push(['W', kebab(id), clean(en), clean(es), clean(more)].join('\t'));
}
for (const [k, v] of Object.entries(out.LABEL_IN)) {
  const [fam, id] = k.split(':');
  lines.push(['L', slug(fam), kebab(id), clean(v[0]), clean(v[1])].join('\t'));
}
for (const id of out.LIB) {
  const g = out.N[id];
  if (!g) { missing.push('drawing:' + id); continue; }
  // (`_px` marks a glyph set pixel by pixel at some sizes; it is not a size.)
  for (const k of Object.keys(g).filter(k => /^\d+$/.test(k)).map(Number).sort((a, b) => a - b)) {
    const body = out.drawing(g[k]).replace(/[\t\r\n]+/g, ' ');
    if (!body) { missing.push(`${id}@${k}`); continue; }
    lines.push(['D', kebab(id), k, body].join('\t'));
  }
}
if (missing.length) console.error('missing: ' + missing.join(' '));
console.error(`${out.FAMILIES.length} families, ${out.LIB.length} icons`);
process.stdout.write(lines.join('\n') + '\n');
