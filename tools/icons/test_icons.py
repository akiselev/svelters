"""Tests for tools/icons/icons.py (IC1). Standard library only: `python3 -m pytest -q tools/icons`."""
import filecmp
import json
import os
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import icons  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
PROTO = os.path.join(HERE, 'proto')
SPECS = icons.load_sizes()
# The svelters-egui loader snapshot test (crates/svelters-egui/tests/icon_loader.rs) renders these build outputs.
EGUI_FIXTURES = os.path.join(REPO, 'crates', 'svelters-egui', 'tests', 'icons', 'build')
EGUI_IDS = 'mcad.extrude,mcad.section,ecad.drc+check,sim.solve,sim.result.contour'

H16 = ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" '
       'stroke-width="1" stroke-linecap="round" stroke-linejoin="round">{}</svg>')
H24 = H16.replace('0 0 16 16', '0 0 24 24').replace('stroke-width="1"', 'stroke-width="1.5"')


def rules(body, icon='mcad.test', size=16, head=None):
    text = (head or (H16 if size == 16 else H24)).format(body)
    return sorted({f.rule for f in icons.lint_svg(text, icon, SPECS[size])})


class LintRules(unittest.TestCase):
    def test_clean(self):
        self.assertEqual(rules('<path d="M1.5 1.5H14.5V14.5H1.5Z"/>'), [])
        self.assertEqual(rules('<path d="M1.5 9.5 7.5 6.5 13.5 9.5"/>'), [])           # 2:1 iso
        self.assertEqual(rules('<path d="M2 2 6 6" stroke-width="2"/>'), [])            # 45 deg copper
        self.assertEqual(rules('<rect x="1" y="1" width="4" height="4" fill="currentColor" stroke="none"/>'), [])

    def test_viewbox_and_size(self):
        self.assertIn('root', rules('<path d="M1.5 1.5H4.5"/>', head=H24, size=16))

    def test_half_pixel(self):
        self.assertEqual(rules('<path d="M1 2H14"/>'), ['half-pixel'])
        self.assertEqual(rules('<path d="M2.5 2.5V10" stroke-width="2"/>'), ['half-pixel'])  # copper: whole px
        self.assertEqual(rules('<circle cx="8" cy="8" r="3"/>'), ['half-pixel'])
        self.assertEqual(rules('<circle cx="8" cy="8" r="3.5"/>'), [])
        self.assertEqual(rules('<rect x="2" y="2" width="5" height="5"/>'), ['half-pixel'])

    def test_stroke_width(self):
        self.assertEqual(rules('<path d="M1.5 2.5H14.5" stroke-width="1.5"/>'), ['stroke-width'])
        self.assertEqual(rules('<path d="M1.5 2.5H14.5" stroke-width="1"/>', size=24), ['stroke-width'])
        clip = ('<defs><clipPath id="c"><rect x="2" y="2" width="10" height="10"/></clipPath></defs>'
                '<g clip-path="url(#c)"><path d="M0 12 12 0" stroke-width="1"/></g>')
        self.assertEqual(rules(clip, size=24), [])                                     # 1px hatching, clipped

    def test_diagonal_angle(self):
        self.assertEqual(rules('<path d="M1.5 1.5 7.5 3.5"/>'), ['diagonal-angle'])

    def test_colour_opacity_raster(self):
        self.assertEqual(rules('<path d="M1.5 1.5H4.5" stroke="#f00"/>'), ['colour'])
        self.assertEqual(rules('<path d="M1.5 1.5H4.5" opacity=".5"/>'), ['opacity'])
        self.assertEqual(rules('<path d="M1.5 1.5H4.5" opacity=".45"/>', icon='sim.result.x'), [])
        self.assertIn('raster', rules('<image href="data:image/png;base64,AAAA" width="4" height="4"/>'))
        mask = ('<defs><mask id="m"><rect width="16" height="16" fill="#fff"/></mask></defs>'
                '<g mask="url(#m)"><path d="M1.5 1.5H4.5"/></g>')
        self.assertEqual(rules(mask), [])

    def test_attributes(self):
        self.assertEqual(rules('<path d="M1.5 1.5H4.5" transform="rotate(45)"/>'), ['attribute'])
        self.assertEqual(rules('<path d="M1.5 1.5H4.5" style="stroke:red"/>'), ['attribute'])
        self.assertEqual(rules('<text x="1" y="1">A</text>'), ['element'])

    def test_dash_gap_bounds(self):
        self.assertEqual(rules('<path d="M0.5 8.5H15.5" stroke-dasharray="4 1 1 1" stroke-linecap="butt"/>'), [])
        self.assertEqual(rules('<path d="M0.5 8.5H15.5" stroke-dasharray="2 2" stroke-linecap="butt"/>'), ['dash'])
        self.assertEqual(rules('<path d="M1.5 4.5H14.5M1.5 5.5H14.5"/>'), ['gap'])
        self.assertEqual(rules('<path d="M1.5 4.5H14.5M1.5 6.5H14.5"/>'), [])
        self.assertEqual(rules('<path d="M1.5 4.5H17.5"/>'), ['bounds'])

    def test_ids(self):
        self.assertEqual(rules('<path d="M1.5 1.5H4.5"/>', icon='Mcad.Extrude'), ['id'])
        self.assertEqual(rules('<path d="M1.5 1.5H4.5"/>', icon='cad.extrude'), ['id'])
        self.assertEqual(rules('<path d="M1.5 1.5H4.5"/>', icon='mcad.extrude+remove'), ['id'])

    def test_rules_come_from_the_size_table(self):
        spec = icons.SizeSpec(32, {'strokes': [{'width': 2, 'centre': 'whole'}], 'min_gap': 2, 'badge': 16})
        svg = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32" fill="none" stroke="currentColor" stroke-width="2">{}</svg>'
        self.assertEqual([f.rule for f in icons.lint_svg(svg.format('<path d="M2 4H30"/>'), 'mcad.x', spec)], [])
        self.assertEqual([f.rule for f in icons.lint_svg(svg.format('<path d="M2 4.5H30"/>'), 'mcad.x', spec)],
                         ['half-pixel'])
        self.assertEqual([f.rule for f in icons.lint_svg(svg.format('<path d="M2 4H30M2 7H30"/>'), 'mcad.x', spec)],
                         ['gap'])
        self.assertEqual(sorted(SPECS), [12, 14, 16, 20, 24, 32])
        self.assertTrue(SPECS[16].required)


class Sets(unittest.TestCase):
    def test_proto_and_badges_lint_clean(self):
        s = icons.IconSet(PROTO)
        self.assertEqual([str(f) for f in icons.lint_badges(s.specs) + icons.lint_set(s)], [])

    def copy(self):
        d = tempfile.mkdtemp()
        self.addCleanup(shutil.rmtree, d)
        shutil.copytree(PROTO, os.path.join(d, 'set'))
        return os.path.join(d, 'set')

    def test_manifest_rules(self):
        p = self.copy()
        with open(os.path.join(p, 'icons.toml'), 'a') as f:
            f.write('\n[icons."mcad.ghost"]\nsizes = [24]\n'
                    '[icons."mcad.chamfer"]\nallow = [{ rule = "gap", reason = "x" }]\n')
        shutil.copy(os.path.join(p, '16', 'mcad.fillet.svg'), os.path.join(p, '16', 'mcad.chamfer.svg'))
        shutil.copy(os.path.join(p, '16', 'mcad.fillet.svg'), os.path.join(p, '16', 'mcad.stray.svg'))
        os.makedirs(os.path.join(p, '18'))
        got = {(f.icon, f.rule) for f in icons.lint_set(icons.IconSet(p))}
        self.assertEqual(got, {('mcad.ghost', 'master'), ('mcad.chamfer', 'waiver-unused'),
                               ('mcad.stray', 'orphan'), ('18/', 'size')})

    def test_frozen(self):
        p = self.copy()
        s = icons.IconSet(p)
        h = s.approval_hash('mcad.fillet')
        with open(os.path.join(p, 'icons.toml')) as f:
            t = f.read().replace('[icons."mcad.fillet"]\n', f'[icons."mcad.fillet"]\napproved = "{h}"\n')
        with open(os.path.join(p, 'icons.toml'), 'w') as f:
            f.write(t)
        self.assertEqual(icons.lint_set(icons.IconSet(p)), [])
        with open(os.path.join(p, '16', 'mcad.fillet.svg'), 'a') as f:
            f.write('\n')
        self.assertEqual([f.rule for f in icons.lint_set(icons.IconSet(p))], ['frozen'])


class Build(unittest.TestCase):
    def test_compose_lints_clean_and_knocks_out(self):
        s = icons.IconSet(PROTO)
        outs = icons.render_outputs(s)
        for size in (16, 24):
            text = outs[('mcad.extrude+remove', size)]
            self.assertIn(f'id="{icons.KNOCKOUT_ID}"', text)
            self.assertIn(f'mask="url(#{icons.KNOCKOUT_ID})"', text)
            found = [f for f in icons.lint_svg(text, 'mcad.extrude', SPECS[size]) if f.rule != 'ref-id']
            self.assertEqual(found, [], text)
        # base_tone: the composed base sits in the tone
        self.assertIn('opacity=".4"', outs[('sim.solve', 16)])
        self.assertEqual(sorted(z for (i, z) in outs if i == 'sim.solve'), [16, 24])
        self.assertNotIn(('ecad.drc+check', 24), outs)          # ecad.drc has no 24 master

    def test_build_tree(self):
        d = tempfile.mkdtemp()
        self.addCleanup(shutil.rmtree, d)
        ids = icons.build(icons.IconSet(PROTO), d)
        self.assertEqual(len(ids), 22)
        for sub in ('16', '24', 'white/16', 'white/24'):
            self.assertTrue(os.path.isdir(os.path.join(d, sub)), sub)
        for name in os.listdir(os.path.join(d, 'white', '16')):
            with open(os.path.join(d, 'white', '16', name)) as f:
                self.assertNotIn('currentColor', f.read(), name)
        with open(os.path.join(d, 'manifest.json')) as f:
            m = json.load(f)
        self.assertEqual(m['licence'], 'CC-BY-4.0')
        self.assertEqual(m['icons']['mcad.extrude+remove']['composed'], {'base': 'mcad.extrude', 'badge': 'remove'})
        self.assertEqual(m['icons']['mcad.extrude']['sizes'], [16, 24])
        with open(os.path.join(d, 'icon_ids.rs')) as f:
            rs = f.read()
        self.assertIn('pub const MCAD_EXTRUDE__REMOVE: &str = "mcad.extrude+remove";', rs)
        with open(os.path.join(d, 'icon_ids.d.ts')) as f:
            self.assertIn("| 'ecad.drc+check'", f.read())

    def test_egui_fixtures_are_current(self):
        """The loader snapshot's SVGs are this tool's output. Regenerate with:
        tools/icons/icons.py build tools/icons/proto crates/svelters-egui/tests/icons/build --only <EGUI_IDS>"""
        d = tempfile.mkdtemp()
        self.addCleanup(shutil.rmtree, d)
        icons.build(icons.IconSet(PROTO), d, set(EGUI_IDS.split(',')))
        cmp = filecmp.dircmp(d, EGUI_FIXTURES)

        def diffs(c, rel=''):
            out = [os.path.join(rel, n) for n in c.left_only + c.right_only + c.diff_files]
            for name, sub in c.subdirs.items():
                out += diffs(sub, os.path.join(rel, name))
            return out

        self.assertEqual(diffs(cmp), [], 'regenerate the egui fixtures (see this test\'s docstring)')


if __name__ == '__main__':
    unittest.main()
