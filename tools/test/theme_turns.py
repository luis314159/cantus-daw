"""Give every test of a cantus-ui test binary that calls theme::apply a turn: one mutex per
binary, taken first thing in every #[test] fn. theme::apply installs its palette and the kit's
tokens for the whole process, so a test applying another theme repaints a test that measures
words or pixels mid-measure (docs/RELEASING.md L5; beta.16's rehearsal on macOS:
fx_limiter's dark-theme demo test cut another test's "GAIN REDUCTION").

    python tools/test/theme_turns.py crates/cantus-ui/tests   prints each file changed

A file already holding `theme_turn` is left alone, so it can run again after new tests are
written. Each turn first applies the default theme: a test found its label only while another
test had left Slate applied (beta.16: inst_pulsus, inst_sampler, modulators). Tests that look for
a label match it in any theme's lettering.
"""
import os
import re
import sys

HELPER = '''
/// This binary's tests take turns (docs/RELEASING.md L5): `theme::apply` installs its palette
/// and the kit's tokens for the whole process, so a test applying a theme repainted another's
/// words mid-measure (beta.16's rehearsal, macOS: fx_limiter's dark demo cut "GAIN REDUCTION").
/// The app has one interface thread; only tests run several.
fn theme_turn() -> std::sync::MutexGuard<'static, ()> {
    static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    // Every test starts in the app's own look, whatever the test before it applied.
    cantus_ui::theme::apply(
        &cantus_ui::theme::default_theme(),
        &egui::Context::default(),
    );
    turn
}
'''


def process(path):
    src = open(path, encoding='utf-8').read()
    crlf = '\r\n' in src
    s = src.replace('\r\n', '\n')
    if 'theme::apply' not in s or 'fn theme_turn()' in s:
        return 0
    lines = s.split('\n')
    out = []
    i = 0
    n_tests = 0
    while i < len(lines):
        line = lines[i]
        out.append(line)
        if line.strip() == '#[test]':
            # Find the fn line, then the line that opens its body.
            j = i + 1
            while j < len(lines) and not re.match(r'\s*(pub\s+)?(async\s+)?fn\s', lines[j]):
                out.append(lines[j])
                j += 1
            if j >= len(lines):
                i = j
                continue
            while j < len(lines) and not lines[j].rstrip().endswith('{'):
                out.append(lines[j])
                j += 1
            out.append(lines[j])
            indent = re.match(r'(\s*)', lines[j]).group(1) + '    '
            out.append(f'{indent}let _theme_turn = theme_turn();')
            n_tests += 1
            i = j + 1
            continue
        i += 1
    if n_tests < 2:
        return 0
    s = '\n'.join(out).rstrip('\n') + '\n' + HELPER
    if crlf:
        s = s.replace('\n', '\r\n')
    open(path, 'w', encoding='utf-8', newline='').write(s)
    return n_tests


d = sys.argv[1]
total = 0
for name in sorted(os.listdir(d)):
    if name.endswith('.rs'):
        n = process(os.path.join(d, name))
        if n:
            print(f'{name}: {n} tests take turns')
            total += 1
print(f'{total} files')
