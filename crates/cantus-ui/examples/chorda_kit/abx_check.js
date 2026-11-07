// The ABX page's counter, checked without a browser: `node abx_check.js abx.html` runs the
// page's script against a stub DOM, presses A, X, A and One, and checks every press is
// counted and saved (round 37: the owner's results recorded `"plays": 0` on every trial).
const fs = require("fs");
const html = fs.readFileSync(process.argv[2], "utf8");
let js = html.split("<script>").pop().split("</script>")[0];
js = js.replace("__FILES__", "{}").replace("__PHRASES__", "[\"ballad\",\"bass\"]");
const store = {};
function node() {
  return { style: {}, appendChild() {}, set innerHTML(v) {}, get innerHTML() { return ""; },
    dataset: {}, className: "", textContent: "", onclick: null, id: "" };
}
const nodes = {};
global.document = {
  documentElement: {},
  getElementById: id => (nodes[id] = nodes[id] || node()),
  querySelectorAll: () => [],
  createElement: () => node(),
  addEventListener() {},
};
global.localStorage = { getItem: k => store[k] || null, setItem: (k, v) => (store[k] = v) };
global.Audio = function () { return { play() {}, pause() {}, currentTime: 0, duration: 1 }; };
global.setInterval = () => 0;
global.confirm = () => true;
global.URL = { createObjectURL: () => "" };
global.Blob = function () {};
js += "\n;globalThis.__t = { play: k => play(k), st: () => st, render: () => render(), set: v => { tab = v; } };";
eval(js);
const t = globalThis.__t;
t.play("a"); t.play("x"); t.play("a");
const first = t.st().abx1[0];
console.log("abx1[0] plays", first.plays, JSON.stringify(first.heard));
if (first.plays !== 3 || first.heard.a !== 2 || first.heard.x !== 1) process.exit(1);
const saved = JSON.parse(Object.values(store)[0]);
if (saved.abx1[0].plays !== 3) { console.log("not saved"); process.exit(1); }
t.set("pref"); t.render(); t.play("one");
console.log("pref[0] plays", t.st().pref[0].plays);
if (t.st().pref[0].plays !== 1) process.exit(1);
console.log("OK");
