import sys, json
from pathlib import Path
from html import escape
sys.path.insert(0,str(Path(__file__).resolve().parent))
from build import ROOT, glyph_word, svg, ORANGE, INK, PAPER, WHITE
import cairosvg
# Type study: 24 installed styles, all outlined in this sheet.
candidates=[]
for family,styles in [('dejavu/DejaVuSansMono',['','-Bold']),('dejavu/DejaVuSans',['','-Bold']),('dejavu/DejaVuSerif',['','-Bold']),('liberation2/LiberationMono',['-Regular','-Bold','-Italic']),('liberation2/LiberationSans',['-Regular','-Bold','-Italic']),('liberation2/LiberationSerif',['-Regular','-Bold','-Italic']),('ubuntu/Ubuntu',['-R','-M','-B','-L','-C','-RI']),('ubuntu/UbuntuMono',['-R','-B','-RI'])]:
    for style in styles:
        path=Path('/usr/share/fonts/truetype')/(family+style+'.ttf')
        if path.exists(): candidates.append(path)
body=f'<rect width="1400" height="{120+len(candidates)*90}" fill="{PAPER}"/><text x="50" y="64" font-family="DejaVu Sans" font-size="30" fill="{INK}">fleetboard / typography study — {len(candidates)} settings</text>'
for i,font in enumerate(candidates):
    word,w,upem=glyph_word(str(font),spacing=False)
    y=130+i*90
    body+=f'<text x="50" y="{y}" font-family="DejaVu Sans" font-size="15" fill="{INK}">{escape(font.stem)}</text><path transform="translate(480 {y+12}) scale({.043*2048/upem:g} {-.043*2048/upem:g})" fill="{ORANGE if font.stem=="DejaVuSansMono-Bold" else INK}" d="{word}"/>'
write=ROOT/'source'/'type-study.svg'; write.write_text(svg('fleetboard typography study',body,1400,120+len(candidates)*90))
cairosvg.svg2png(url=str(write),write_to=str(write.with_suffix('.png')))

def placed(path,x,y,w,h):
    text=Path(path).read_text(); start=text.index('>')+1
    body=text[start:text.rfind('</svg>')]
    import re
    vb=re.search(r'viewBox="([^"]+)"',text).group(1)
    return f'<svg x="{x}" y="{y}" width="{w}" height="{h}" viewBox="{vb}">{body}</svg>'

# One concise identity overview. Labels are presentation text, marks remain paths.
b=f'<rect width="1600" height="1120" fill="{PAPER}"/>'
b+=f'<text x="80" y="76" font-family="DejaVu Sans Mono" font-size="18" fill="{ORANGE}">FLEETBOARD / PARALLEL F</text>'
b+=f'<text x="80" y="132" font-family="DejaVu Sans" font-size="32" fill="{INK}">Independent work streams. One shared board.</text>'
b+=placed(ROOT/'digital'/'fleetboard-horizontal-color.svg',45,185,1160,260)
b+=f'<line x1="80" y1="476" x2="1520" y2="476" stroke="#D7D7CE"/>'
b+=f'<rect x="80" y="520" width="700" height="270" rx="16" fill="{INK}"/>'
b+=placed(ROOT/'digital'/'fleetboard-horizontal-white.svg',110,563,640,160)
b+=f'<text x="110" y="759" font-family="DejaVu Sans Mono" font-size="15" fill="{PAPER}">REVERSED / OPTICALLY LIGHTENED SYMBOL</text>'
b+=placed(ROOT/'digital'/'fleetboard-symbol-ink.svg',840,518,232,232)
b+=placed(ROOT/'digital'/'fleetboard-app-icon.svg',1160,530,208,208)
b+=f'<text x="850" y="779" font-family="DejaVu Sans Mono" font-size="15" fill="{INK}">ONE COLOUR</text><text x="1170" y="779" font-family="DejaVu Sans Mono" font-size="15" fill="{INK}">APP ICON</text>'
for i,(name,col) in enumerate([('EMBER',ORANGE),('INK',INK),('PAPER',PAPER)]):
    x=80+i*350
    b+=f'<rect x="{x}" y="850" width="310" height="96" rx="8" fill="{col}" stroke="#D7D7CE"/><text x="{x}" y="980" font-family="DejaVu Sans Mono" font-size="16" fill="{INK}">{name} / {col}</text>'
b+=f'<text x="1190" y="866" font-family="DejaVu Sans Mono" font-size="15" fill="{INK}">SMALL-SIZE CUT</text>'
for i,s in enumerate([16,24,32,48]): b+=placed(ROOT/'web'/'favicon.svg',1190+i*74,904,s,s)
b+=f'<text x="1190" y="980" font-family="DejaVu Sans Mono" font-size="15" fill="{INK}">16 / 24 / 32 / 48 PX</text>'
b+=f'<text x="80" y="1070" font-family="DejaVu Sans" font-size="17" fill="#626962">SVG masters · Outlined type · Transparent PNGs · Web icons · Usage guide</text>'
out=ROOT/'presentation'/'identity-overview.svg';out.write_text(svg('fleetboard — Parallel F identity',b,1600,1120));cairosvg.svg2png(url=str(out),write_to=str(out.with_suffix('.png')))
# RGB contrast calculations, unprofiled CMYK estimates, no invented Pantone match.
def rgb(h): return [int(h[i:i+2],16)/255 for i in [1,3,5]]
def lum(h):
    t=[v/12.92 if v<=.04045 else ((v+.055)/1.055)**2.4 for v in rgb(h)]
    return sum(v*c for v,c in zip(t,[.2126,.7152,.0722]))
def contrast(a,b):
    a,b=sorted([lum(a),lum(b)]);return round((b+.05)/(a+.05),2)
for name,col in [('Ember',ORANGE),('Ink',INK),('Paper',PAPER),('White',WHITE)]:
    r,g,b=rgb(col);k=1-max(r,g,b)
    cmyk=[round(100*(1-v-k)/(1-k)) if k<1 else 0 for v in [r,g,b]]+[round(100*k)]
    print(name,col,'CMYK',cmyk)
print('CONTRAST',json.dumps({f'{a}/{b}':contrast(a,b) for a,b in [(ORANGE,WHITE),(ORANGE,PAPER),(INK,PAPER),(WHITE,INK),(ORANGE,INK)]}))
