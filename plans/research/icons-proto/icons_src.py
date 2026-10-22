"""Hand-written prototype icons for ICONS.md section 4 (v3).

    python3 icons_src.py <icons-proto-dir>   # writes 16/<id>.svg and 24/<id>.svg
"""
import os, sys

HEAD16 = ('<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" '
          'stroke="currentColor" stroke-width="1" stroke-linecap="round" stroke-linejoin="round">')
F = 'fill="currentColor" stroke="none"'          # solid fill, no stroke
T = 'fill="currentColor" fill-opacity=".4"'      # secondary tone fill (keeps the stroke)
BADGE_MASK = ('<defs><mask id="k"><rect width="16" height="16" fill="#fff"/>'
              '<rect x="8" y="8" width="8" height="8" rx="1" fill="#000"/></mask></defs>')

I = {}

# ---- MCAD ----------------------------------------------------------------
I['mcad.extrude'] = f'''
<path d="M1.5 9.5 7.5 6.5 13.5 9.5 7.5 12.5Z" {T}/>
<path d="M1.5 9.5V11.5L7.5 14.5 13.5 11.5V9.5M7.5 12.5V14.5"/>
<path d="M7.5 9.5V1.5M5 4 7.5 1.5 10 4"/>'''

I['mcad.revolve'] = f'''
<path d="M0.5 10.5H15.5" stroke-dasharray="4 1 1 1" stroke-linecap="butt"/>
<path d="M1.5 10.5V3.5H5.5V6.5H8.5V10.5" {T}/>
<path d="M11.5 7A2.5 3.5 0 0 1 11.5 14"/>
<path d="M9.5 14 12 12.25V15.75Z" {F}/>'''

I['mcad.fillet'] = f'''
<path d="M2.5 14.5V2.5H6.5A7 7 0 0 1 13.5 9.5V14.5Z"/>
<path d="M6.5 9.5 11 5"/>
<path d="M11.45 4.55 8.5 5 11 7.5Z" {F}/>'''

I['mcad.sketch'] = f'''
<path d="M2.5 13.5H12.5M2.5 13.5V7.5H6.5"/>
<rect x="1.5" y="12.5" width="2" height="2"/><rect x="11.5" y="12.5" width="2" height="2"/>
<rect x="1.5" y="6.5" width="2" height="2"/>
<path d="M10.5 2.5 12.5 4.5 9.5 7.5 7 8 7.5 5.5Z"/>'''

I['mcad.constraint.coincident'] = f'''
<path d="M1.5 14.5 7.5 8.5H14.5" opacity=".4"/>
<circle cx="7.5" cy="8.5" r="2.5" {F}/>'''

I['mcad.mate'] = f'''
<path d="M2.5 1.5H13.5V4.5H10.5V7.5H5.5V4.5H2.5Z" {T}/>
<path d="M1.5 9.5H5.5V12.5H10.5V9.5H14.5V14.5H1.5Z"/>'''

I['mcad.section'] = f'''
<defs><clipPath id="c"><path d="M4.5 7 10.5 4V10L4.5 13Z"/></clipPath></defs>
<path d="M1.5 5.5 7.5 2.5 10.5 4V10L4.5 13 1.5 11.5Z"/>
<path d="M1.5 5.5 4.5 7V13M4.5 7 10.5 4"/>
<g clip-path="url(#c)"><path d="M3 12 11 4M3 16 12 7M6 17 12 11"/></g>'''

# ---- ECAD ----------------------------------------------------------------
I['ecad.via'] = f'''
<path {F} fill-rule="evenodd" d="M10 3a5 5 0 1 0 0 10a5 5 0 1 0 0-10Zm0 3a2 2 0 1 1 0 4a2 2 0 1 1 0-4Z"/>
<path d="M1 8H6" stroke-width="2" stroke-linecap="butt"/>'''

I['ecad.trace'] = f'''
<rect x="1" y="11" width="4" height="4" {F}/><rect x="11" y="1" width="4" height="4" {F}/>
<path d="M3 11V9L9 3H11" stroke-width="2" stroke-linecap="butt"/>'''

I['ecad.footprint'] = f'''
<rect x="5.5" y="1.5" width="5" height="13"/>
<path d="M1 2h3v2h-3zM1 7h3v2h-3zM1 12h3v2h-3zM12 2h3v2h-3zM12 7h3v2h-3zM12 12h3v2h-3z" {F}/>
<circle cx="7.5" cy="3.5" r="1" {F}/>'''

I['ecad.net-label'] = f'''
<path d="M0.5 7.5H5.5"/>
<path d="M5.5 7.5 8.5 4.5H14.5V10.5H8.5Z"/>
<path d="M10.5 7.5H12.5" opacity=".4"/>'''

I['ecad.copper-pour'] = f'''
<path fill-rule="evenodd" {T} d="M1.5 1.5H14.5V14.5H5.5L1.5 10.5ZM8 4.5a3.5 3.5 0 1 0 0 7a3.5 3.5 0 1 0 0-7Z" stroke="none"/>
<path d="M1.5 1.5H14.5V14.5H5.5L1.5 10.5Z"/>
<circle cx="8" cy="8" r="2" {F}/>
<path d="M8 4.5V6M8 10V11.5M4.5 8H6M10 8H11.5" stroke-width="2" stroke-linecap="butt" opacity=".4"/>'''

I['ecad.drc'] = f'''{BADGE_MASK}
<g mask="url(#k)">
<path d="M3 1V15M13 1V15" stroke-width="2" stroke-linecap="butt"/>
<path d="M5.5 5.5H10.5M6.5 4.5 5.5 5.5 6.5 6.5M9.5 4.5 10.5 5.5 9.5 6.5"/>
</g>
<path d="M9.5 12.5 11.5 14.5 14.5 10.5"/>'''

# ---- Simulation ----------------------------------------------------------
I['sim.mesh'] = f'''
<path d="M1.5 1.5H13.5V13.5H1.5Z"/>
<path d="M1.5 7.5H13.5M7.5 1.5V13.5M1.5 7.5 7.5 1.5M1.5 13.5 13.5 1.5M7.5 13.5 13.5 7.5"/>'''

I['sim.bc.fixed'] = f'''
<rect x="3.5" y="3.5" width="9" height="7" {T}/>
<path d="M1.5 10.5H14.5"/>
<path d="M4.5 11.5 2.5 13.5M8.5 11.5 6.5 13.5M12.5 11.5 10.5 13.5"/>'''

I['sim.load.force'] = f'''
<rect x="1.5" y="10.5" width="13" height="4" {T}/>
<path d="M7.5 1.5V6.5"/>
<path d="M4.5 6 10.5 6 7.5 10Z" {F}/>'''

I['sim.probe'] = f'''
<circle cx="4.5" cy="11.5" r="3"/>
<circle cx="4.5" cy="11.5" r="1" {F}/>
<path d="M6.5 9.5 8.5 7.5"/>
<rect x="8.5" y="1.5" width="6" height="6"/>
<path d="M10.5 4.5H12.5" opacity=".4"/>'''

I['sim.solve'] = f'''{BADGE_MASK}
<g mask="url(#k)" opacity=".4">
<path d="M1.5 1.5H14.5V14.5H1.5Z"/>
<path d="M1.5 7.5H14.5M7.5 1.5V14.5M1.5 7.5 7.5 1.5M7.5 7.5 14.5 1.5M1.5 14.5 7.5 7.5"/>
</g>
<path d="M10 9V15L15 12Z" {F}/>'''

I['sim.result.contour'] = f'''
<defs><clipPath id="c"><rect x="2" y="2" width="12" height="12"/></clipPath></defs>
<g clip-path="url(#c)" stroke="none" fill="currentColor">
<circle cx="2" cy="14" r="12" fill-opacity=".2"/>
<circle cx="2" cy="14" r="8.5" fill-opacity=".45"/>
<circle cx="2" cy="14" r="5"/>
</g>
<rect x="1.5" y="1.5" width="13" height="13"/>'''

# ---- Shared --------------------------------------------------------------
I['ecad.layer-stack'] = f'''
<path d="M1 2h5v2h-5zM9 2h6v2h-6zM1 7h14v2h-14zM1 12h14v2h-14z" {F}/>
<path d="M1 4h14v3h-14zM1 9h14v3h-14z" fill="currentColor" fill-opacity=".4" stroke="none"/>'''


# ---- 24px masters (stroke 1.5), a subset -----------------------------------
HEAD24 = ('<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" '
          'stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">')
J = {}
J['mcad.extrude'] = f'''
<path d="M1.5 14.5 11.5 9.5 21.5 14.5 11.5 19.5Z" {T}/>
<path d="M1.5 14.5V17.5L11.5 22.5 21.5 17.5V14.5M11.5 19.5V22.5"/>
<path d="M11.5 14.5V1.5M8 5 11.5 1.5 15 5"/>'''

J['mcad.revolve'] = f'''
<path d="M0.5 16.5H23.5" stroke-dasharray="6 1.5 1.5 1.5" stroke-linecap="butt"/>
<path d="M2.5 16.5V4.5H7.5V9.5H11.5V16.5" {T}/>
<path d="M16.5 11.5A4 5 0 0 1 16.5 21.5"/>
<path d="M13.5 21.5 17 19.25V23.75Z" {F}/>'''

J['ecad.via'] = f'''
<path {F} fill-rule="evenodd" d="M15.5 5.5a7 7 0 1 0 0 14a7 7 0 1 0 0-14Zm0 4a3 3 0 1 1 0 6a3 3 0 1 1 0-6Z"/>
<path d="M1 12.5H9" stroke-width="3" stroke-linecap="butt"/>'''

J['sim.mesh'] = '''
<path d="M2.5 2.5H20.5V20.5H2.5Z"/>
<path d="M2.5 11.5H20.5M11.5 2.5V20.5M2.5 11.5 11.5 2.5M2.5 20.5 20.5 2.5M11.5 20.5 20.5 11.5"/>'''

J['sim.bc.fixed'] = f'''
<rect x="4.5" y="4.5" width="14" height="10" {T}/>
<path d="M1.5 14.5H21.5"/>
<path d="M6.5 16.5 3.5 19.5M11.5 16.5 8.5 19.5M16.5 16.5 13.5 19.5M21.5 16.5 18.5 19.5"/>'''

J['sim.result.contour'] = '''
<defs><clipPath id="c"><rect x="2.5" y="2.5" width="18" height="18"/></clipPath></defs>
<g clip-path="url(#c)" stroke="none" fill="currentColor">
<circle cx="2.5" cy="20.5" r="18" fill-opacity=".2"/>
<circle cx="2.5" cy="20.5" r="12.5" fill-opacity=".45"/>
<circle cx="2.5" cy="20.5" r="7"/>
</g>
<rect x="2.5" y="2.5" width="18" height="18"/>'''


def main():
    out = sys.argv[1]
    for sub, head, icons in (('16', HEAD16, I), ('24', HEAD24, J)):
        os.makedirs(os.path.join(out, sub), exist_ok=True)
        for name, body in icons.items():
            with open(os.path.join(out, sub, name + '.svg'), 'w') as f:
                f.write(head + body.strip().replace('\n', '') + '</svg>\n')
    print(len(I), '16px icons,', len(J), '24px masters')


if __name__ == '__main__':
    main()
