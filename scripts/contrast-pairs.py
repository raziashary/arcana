"""Static contrast scan (P6-01): text and background tokens declared together in one style object or
className. Reports pairs under the floor (4.5, or 3.0 for text-dim and text-4) and exits 1 if any.
Run from the repo root: python scripts/contrast-pairs.py"""
import re, glob, collections

css = open('src/styles/globals.css', encoding='utf-8').read()
root = css[css.index(':root'):]
root = root[:root.index('\n}')]
tok = {m.group(1): m.group(2).split('/*')[0].strip() for m in re.finditer(r'--([a-z0-9-]+):\s*([^;]+);', root)}


def parse(v):
    v = v.strip()
    if v.startswith('var('):
        return parse(tok[v[4:v.index(')')].lstrip('-')])
    if v.startswith('#'):
        h = v[1:]
        if len(h) == 3:
            h = ''.join(c * 2 for c in h)
        return int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16), 1.0
    m = re.match(r'rgba?\(([^)]+)\)', v)
    if m:
        p = [float(x) for x in m.group(1).split(',')]
        return p[0], p[1], p[2], p[3] if len(p) > 3 else 1.0
    m = re.match(r'color-mix\(in srgb,\s*var\(--([a-z0-9-]+)\)\s*(\d+)%,\s*transparent\)', v)
    if m:
        r, g, b, a = parse('var(--%s)' % m.group(1))
        return r, g, b, a * int(m.group(2)) / 100
    raise KeyError(v)


def over(f, b):
    return tuple(f[3] * x + (1 - f[3]) * y for x, y in zip(f[:3], b[:3]))


def lum(c):
    def ch(x):
        x /= 255
        return x / 12.92 if x <= 0.03928 else ((x + 0.055) / 1.055) ** 2.4
    return 0.2126 * ch(c[0]) + 0.7152 * ch(c[1]) + 0.0722 * ch(c[2])


PAGE = parse('var(--bg-app)')


def ratio(fg, bg):
    b = parse('var(--%s)' % bg)
    b = over(b, PAGE) + (1.0,) if b[3] < 1 else b
    f = over(parse('var(--%s)' % fg), b)
    l1, l2 = lum(f), lum(b[:3])
    return (max(l1, l2) + .05) / (min(l1, l2) + .05)


results = collections.defaultdict(list)
checked = 0
for path in glob.glob('src/**/*.tsx', recursive=True):
    if '.test.' in path:
        continue
    text = open(path, encoding='utf-8').read()
    # style objects: background + color within the same {...} on up to 6 lines
    for m in re.finditer(r'\{[^{}]{0,400}\}', text, re.S):
        blk = m.group(0)
        bg = re.search(r"background(?:Color)?:\s*[`']?(?:var\(--([a-z0-9-]+)\))", blk)
        fg = re.search(r"(?<![a-zA-Z])color:\s*[`']?var\(--([a-z0-9-]+)\)", blk)
        if bg and fg:
            line = text[:m.start()].count('\n') + 1
            try:
                r = ratio(fg.group(1), bg.group(1))
            except KeyError:
                continue
            checked += 1
            need = 3.0 if fg.group(1) in ('text-dim', 'text-4') else 4.5
            if r < need:
                results[(fg.group(1), bg.group(1), round(r, 2))].append('%s:%d' % (path.replace('\\', '/'), line))
    # tailwind: bg-[color:var(--x)] ... text-[color:var(--y)] in the same className string
    for m in re.finditer(r'className=[`"\']([^`"\']*)[`"\']', text):
        cls = m.group(1)
        bg = re.search(r'bg-\[color:var\(--([a-z0-9-]+)\)\]', cls)
        fg = re.search(r'text-\[color:var\(--([a-z0-9-]+)\)\]', cls)
        if bg and fg:
            line = text[:m.start()].count('\n') + 1
            try:
                r = ratio(fg.group(1), bg.group(1))
            except KeyError:
                continue
            checked += 1
            need = 3.0 if fg.group(1) in ('text-dim', 'text-4') else 4.5
            if r < need:
                results[(fg.group(1), bg.group(1), round(r, 2))].append('%s:%d' % (path.replace('\\', '/'), line))

print('pairs checked:', checked)
for (fg, bg, r), where in sorted(results.items(), key=lambda kv: kv[0][2]):
    print('%-14s on %-14s %.2f  x%d  e.g. %s' % (fg, bg, r, len(where), ', '.join(where[:3])))

raise SystemExit(1 if results else 0)
