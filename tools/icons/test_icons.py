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
SAMPLES = os.path.join(HERE, 'samples')
STYLE = icons.Style()
SPECS = STYLE.specs
# The svelters-egui loader snapshot (crates/svelters-egui/tests/icon_loader.rs) renders these build outputs.
EGUI_FIXTURES = os.path.join(REPO, 'crates', 'svelters-egui', 'tests', 'icons', 'build')
EGUI_IDS = 'mcad.extrude,mcad.section,sim.result,mcad.extrude+remove,chrome.warning'


def head(size):
    w = icons._fmt(SPECS[size].stroke)
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" viewBox="0 0 {size} {size}" '
            f'fill="none" stroke="currentColor" stroke-width="{w}" stroke-linecap="round" stroke-linejoin="round">'
            '{}</svg>')


def rules(body, icon='mcad.test', size=16, svg=None, **kw):
    text = svg or head(size).format(body)
    return sorted({f.rule for f in icons.lint_svg(text, icon, SPECS[size], STYLE, **kw)})


F = 'fill="currentColor" stroke="none"'
TONE = 'class="subject" fill="currentColor" fill-opacity=".4"'


class LintRules(unittest.TestCase):
    def test_clean(self):
        self.assertEqual(rules('<path d="M1.5 1.5H13.5V13.5H1.5Z"/>'), [])
        self.assertEqual(rules('<path d="M1.5 9.5 7.5 6.5 13.5 9.5"/>'), [])                 # 2:1 iso
        self.assertEqual(rules('<path d="M3 3 7 7" stroke-width="2" stroke-linecap="butt"/>'), [])  # copper
        self.assertEqual(rules(f'<rect x="2" y="2" width="4" height="4" {F}/>'), [])
        self.assertEqual(rules(f'<path {TONE} d="M1.5 9.5 7.5 6.5 13.5 9.5 7.5 12.5Z"/>'), [])

    def test_l1_root_and_terminals(self):
        self.assertEqual(rules('', svg=head(16).format('').replace('stroke-linecap="round"', 'stroke-linecap="square"')), ['L1'])
        self.assertEqual(rules('', svg=head(16).format('').replace(' width="16" height="16"', '')), ['L1'])
        self.assertEqual(rules('<path d="M2.5 2.5H12.5" stroke-linecap="square"/>'), ['L1'])
        self.assertIn('L1', rules('', size=24, svg=head(16).format('<path d="M2.5 2.5H5.5"/>')))  # a 16 in 24/

    def test_l2_l3_elements_and_colours(self):
        self.assertEqual(rules('<path d="M2.5 2.5H5.5" transform="rotate(45)"/>'), ['L2'])
        self.assertEqual(rules('<path d="M2.5 2.5H5.5" style="stroke:red"/>'), ['L2'])
        self.assertEqual(rules('<ellipse cx="8" cy="8" rx="3" ry="2"/>'), ['L2'])
        self.assertEqual(rules('<image href="data:image/png;base64,AAAA" width="4" height="4"/>'), ['L2'])
        self.assertEqual(rules('<path d="M2.5 2.5H5.5" stroke="#f00"/>'), ['L3'])
        mask = ('<defs><mask id="m"><rect width="16" height="16" fill="#fff"/></mask></defs>'
                '<g mask="url(#m)"><path d="M2.5 2.5H5.5"/></g>')
        self.assertEqual(rules(mask), [])

    def test_l4_classes(self):
        self.assertEqual(rules('<path class="glow" d="M2.5 2.5H5.5"/>'), ['L4'])
        self.assertEqual(rules('<path class="accent" d="M2.5 2.5H5.5"/>', icon='mcad.x'), ['L4'])
        self.assertEqual(rules('<path class="accent" d="M2.5 2.5H5.5"/>', icon='chrome.warning'), [])
        self.assertEqual(rules('<path class="badge" d="M2.5 2.5H5.5"/>'), ['L4'])
        self.assertEqual(rules('<path class="badge" d="M2.5 2.5H5.5"/>', built=True), [])
        self.assertEqual(rules('<path class="accent" fill="currentColor" d="M2.5 2.5H5.5V5.5H2.5Z"/>', icon='chrome.run'), ['L4'])

    def test_l5_tone_fringe_corners(self):
        self.assertEqual(rules('<path d="M2.5 2.5H5.5" opacity=".4"/>'), ['L5'])               # tone outside subject
        self.assertEqual(rules('<path class="subject" d="M2.5 2.5H5.5" opacity=".5"/>'), ['L5'])
        self.assertEqual(rules(f'<path {TONE} d="M1.5 6.5 5.5 4.5 9.5 6.5 5.5 8.5Z"/>', size=12), ['L5'])  # no tone at 12
        self.assertEqual(rules('<path d="M2.5 2.5H5.5" opacity=".45"/>', icon='sim.result.x'), [])
        self.assertEqual(rules('<rect x="2.5" y="2.5" width="8" height="8" rx="1"/>'), ['L5'])  # O2: square corners

    def test_l6_half_pixel_and_fill_edges(self):
        self.assertEqual(rules('<path d="M2 2H12"/>'), ['L6'])
        self.assertEqual(rules('<path d="M2.5 2.5V10" stroke-width="2"/>'), ['L6'])           # copper: whole px
        self.assertEqual(rules('<circle cx="8" cy="8" r="3"/>'), ['L6'])
        self.assertEqual(rules('<circle cx="8" cy="8" r="3.5"/>'), [])
        self.assertEqual(rules(f'<rect x="2.5" y="2" width="4" height="4" {F}/>'), ['L6'])     # fill edge on .5
        self.assertEqual(rules('<path d="M2.5 2.5H12.5"/>', size=20, svg=head(20).format('<path d="M2.5 2.5H12.5"/>')), [])

    def test_l7_l9_angles_and_widths(self):
        self.assertEqual(rules('<path d="M1.5 1.5 7.5 3.5"/>'), ['L7'])
        self.assertEqual(rules('<path d="M1.5 1.5 7.5 3.5"/>', free_angles=True), [])
        self.assertEqual(rules('<path d="M1.5 2.5H13.5" stroke-width="1.5"/>'), ['L9'])
        self.assertEqual(rules('<path d="M1.5 2.5H13.5" stroke-width="1"/>', size=24), ['L9'])
        clip = ('<defs><clipPath id="c"><rect x="2" y="2" width="10" height="10"/></clipPath></defs>'
                '<g clip-path="url(#c)"><path d="M0 12 12 0" stroke-width="1"/></g>')
        self.assertEqual(rules(clip, size=24), [])                                            # 1px hatch, clipped

    def test_l8_live_area(self):
        self.assertEqual(rules('<path d="M0.5 4.5V10.5H10.5"/>'), ['L8'])                     # an interior vertex
        self.assertEqual(rules('<path d="M7.5 0.5V10.5"/>'), [])                              # a round-cap end
        self.assertEqual(rules(f'<path {F} d="M7.5 0 10.5 3H4.5Z"/>'), [])                    # a head touches
        self.assertEqual(rules('<path d="M1.5 3.5V17.5"/>'), ['L8'])                          # off the canvas

    def test_l10_gaps_and_counters(self):
        self.assertEqual(rules('<path d="M1.5 4.5H13.5M1.5 5.5H13.5"/>'), ['L10'])
        self.assertEqual(rules('<path d="M1.5 4.5H13.5M1.5 6.5H13.5"/>'), [])
        self.assertEqual(rules('<path d="M2.5 4.5H18.5M2.5 6.5H18.5"/>', size=24), ['L10'])   # 24 keeps 1.5
        self.assertEqual(rules('<rect x="2.5" y="2.5" width="2" height="2"/>'), ['L10'])
        self.assertEqual(rules('<circle cx="8" cy="8" r="1.5"/>'), [])                         # 2px counter

    def test_l11_arrowheads(self):
        self.assertEqual(rules(f'<path {F} d="M7.5 1 L10.5 4 L4.5 4Z"/>'), [])                 # half-base 3 at 16
        self.assertEqual(rules(f'<path {F} d="M7.5 1 L9.5 3 L5.5 3Z"/>'), ['L11'])
        self.assertEqual(rules(f'<path {F} d="M5 3 14 7.5 5 12Z"/>'), [])                       # run: not a head

    def test_l13_and_ids(self):
        self.assertEqual(rules('<path d="M2.5 2.5H5.5"/>' * 60), ['L13'])
        self.assertEqual(rules('<path d="M2.5 2.5H5.5"/>', icon='Mcad.Extrude'), ['id'])
        self.assertEqual(rules('<path d="M2.5 2.5H5.5"/>', icon='cad.extrude'), ['id'])
        self.assertEqual(rules('<path d="M2.5 2.5H5.5"/>', icon='mcad.extrude+remove'), ['id'])

    def test_size_table(self):
        self.assertEqual(sorted(SPECS), [12, 14, 16, 20, 24, 32])
        self.assertEqual([SPECS[z].stroke for z in (12, 14, 16, 20, 24)], [1, 1, 1, 1.25, 1.5])
        self.assertEqual([SPECS[z].arrowhead for z in (12, 14, 16, 20, 24)], [2, 2.5, 3, 3.5, 4.5])
        self.assertEqual([icons.phase(w) for w in (1, 1.25, 1.5, 2, 3, 4)], [.5, .5, .5, 0, .5, 0])


class Sets(unittest.TestCase):
    def test_samples_and_badges_lint_clean(self):
        s = icons.IconSet(SAMPLES)
        self.assertEqual([str(f) for f in icons.lint_badges(s.style) + icons.lint_set(s)], [])

    def copy(self):
        d = tempfile.mkdtemp()
        self.addCleanup(shutil.rmtree, d)
        shutil.copytree(SAMPLES, os.path.join(d, 'set'))
        return os.path.join(d, 'set')

    def test_manifest_rules(self):
        p = self.copy()
        with open(os.path.join(p, 'icons.toml'), 'a') as f:
            f.write('\n[icons."mcad.ghost"]\nuses = ["strip"]\n'
                    '[icons."mcad.chamfer"]\nallow = [{ rule = "L10", reason = "x" }]\n')
        for name in ('mcad.ghost', 'mcad.chamfer', 'mcad.stray'):
            shutil.copy(os.path.join(p, '16', 'mcad.fillet.svg'), os.path.join(p, '16', name + '.svg'))
        os.makedirs(os.path.join(p, '18'))
        got = {(f.icon, f.rule) for f in icons.lint_set(icons.IconSet(p))}
        self.assertEqual(got, {('mcad.ghost', 'L12'), ('mcad.chamfer', 'waiver-unused'),
                               ('mcad.stray', 'orphan'), ('18/', 'size')})

    def test_required_masters(self):
        s = icons.IconSet(SAMPLES)
        self.assertEqual(s.required('chrome.close'), [16, 20, 24])
        self.assertEqual(s.required('mcad.extrude'), [12, 14, 16, 20, 24])
        self.assertEqual(s.required('mcad.fillet'), [16, 24])

    def test_frozen(self):
        p = self.copy()
        h = icons.IconSet(p).approval_hash('mcad.fillet')
        with open(os.path.join(p, 'icons.toml')) as f:
            t = f.read().replace('[icons."mcad.fillet"]\n', f'[icons."mcad.fillet"]\napproved = "{h}"\n')
        with open(os.path.join(p, 'icons.toml'), 'w') as f:
            f.write(t)
        self.assertEqual(icons.lint_set(icons.IconSet(p)), [])
        with open(os.path.join(p, '16', 'mcad.fillet.svg'), 'a') as f:
            f.write('\n')
        self.assertEqual([f.rule for f in icons.lint_set(icons.IconSet(p))], ['frozen'])


class Build(unittest.TestCase):
    def test_compose_clears_the_cell(self):
        s = icons.IconSet(SAMPLES)
        outs = icons.render_outputs(s)
        for size, cell in ((16, 8), (24, 12)):
            text = outs[('mcad.extrude+remove', size)]
            self.assertIn(f'<rect x="{size - cell}" y="{size - cell}" width="{cell}" height="{cell}" fill="#000" stroke="none" />', text)
            self.assertIn(f'mask="url(#{icons.KNOCKOUT_ID})"', text)
            self.assertIn('class="badge"', text)
            self.assertEqual(icons.lint_svg(text, 'mcad.extrude+remove', SPECS[size], STYLE, built=True), [])
        self.assertIn('class="subject" opacity=".4"', outs[('sim.solve', 16)])     # base_tone
        self.assertNotIn(('mcad.extrude+remove', 12), outs)                         # no badge cell at 12/14
        self.assertNotIn(('mcad.extrude+remove', 20), outs)                         # no 20 glyphs drawn yet

    def test_high_contrast_and_white(self):
        t = '<path class="subject" fill="currentColor" fill-opacity=".4"/><g opacity=".4"/>'
        self.assertEqual(icons.high_contrast(t, 0.6),
                         '<path class="subject" fill="currentColor" fill-opacity="0.6"/><g opacity="0.6"/>')
        self.assertNotIn('currentColor', icons.white(t))

    def test_pick_master(self):
        """ICON-STYLE §5.3's table for a 16pt icon with the 16, 20 and 24 masters."""
        m = [16, 20, 24]
        got = {p: icons.pick_master(p, m) for p in (16, 18, 19, 20, 21, 22, 24, 26, 27, 29, 32)}
        self.assertEqual(got[16], (16, 16, 0, 'exact'))
        self.assertEqual(got[18], (16, 16, 1, 'padded'))
        self.assertEqual(got[19], (20, 19, 0, 'scaled'))
        self.assertEqual(got[21], (20, 20, 0, 'padded'))
        self.assertEqual(got[22], (20, 20, 1, 'padded'))
        self.assertEqual(got[24], (24, 24, 0, 'exact'))
        self.assertEqual(got[26], (24, 24, 1, 'padded'))
        self.assertEqual(got[27], (24, 27, 0, 'scaled'))
        self.assertEqual(got[29], (16, 29, 0, 'scaled'))                            # 16x2, scaled
        self.assertEqual(got[32], (16, 32, 0, 'x2'))

    def test_build_tree(self):
        d = tempfile.mkdtemp()
        self.addCleanup(shutil.rmtree, d)
        ids = icons.build(icons.IconSet(SAMPLES), d)
        self.assertEqual(len(ids), 31)
        for sub in ('12', '16', '20', '24', 'hc/16', 'white/16', 'white/hc/24'):
            self.assertTrue(os.path.isdir(os.path.join(d, sub)), sub)
        for name in os.listdir(os.path.join(d, 'white', '16')):
            with open(os.path.join(d, 'white', '16', name)) as f:
                self.assertNotIn('currentColor', f.read(), name)
        with open(os.path.join(d, 'hc', '16', 'mcad.extrude.svg')) as f:
            self.assertIn('fill-opacity="0.6"', f.read())
        with open(os.path.join(d, 'manifest.json')) as f:
            m = json.load(f)
        self.assertEqual(m['licence'], 'CC-BY-4.0')
        self.assertEqual(m['icons']['mcad.extrude+remove']['composed'], {'base': 'mcad.extrude', 'badge': 'remove'})
        self.assertEqual(m['icons']['mcad.extrude']['sizes'], [12, 14, 16, 20, 24])
        with open(os.path.join(d, 'icon_ids.rs')) as f:
            self.assertIn('pub const MCAD_EXTRUDE__REMOVE: &str = "mcad.extrude+remove";', f.read())
        with open(os.path.join(d, 'icon_ids.d.ts')) as f:
            self.assertIn("| 'chrome.file+lock'", f.read())

    def test_egui_fixtures_are_current(self):
        """The loader snapshot's SVGs are this tool's output. Regenerate with:
        tools/icons/icons.py build tools/icons/samples crates/svelters-egui/tests/icons/build --only <EGUI_IDS>"""
        d = tempfile.mkdtemp()
        self.addCleanup(shutil.rmtree, d)
        icons.build(icons.IconSet(SAMPLES), d, set(EGUI_IDS.split(',')))

        def diffs(c, rel=''):
            out = [os.path.join(rel, n) for n in c.left_only + c.right_only + c.diff_files]
            for name, sub in c.subdirs.items():
                out += diffs(sub, os.path.join(rel, name))
            return out

        self.assertEqual(diffs(filecmp.dircmp(d, EGUI_FIXTURES)), [],
                         'regenerate the egui fixtures (see this test\'s docstring)')


if __name__ == '__main__':
    unittest.main()
