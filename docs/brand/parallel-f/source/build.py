#!/usr/bin/env python3
"""Rebuild vector masters and exports. Requires fonttools, cairosvg, Pillow.
No font binaries are distributed; --font may select a local DejaVuSansMono-Bold.ttf.
"""
import argparse
import json
from pathlib import Path
from html import escape
from fontTools.ttLib import TTFont
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.pens.boundsPen import BoundsPen
import cairosvg
from PIL import Image

ROOT=Path(__file__).resolve().parents[1]
ORANGE='#B84A2A'; INK='#202522'; PAPER='#F5F3EE'; WHITE='#FFFFFF'
MARK='M40 40H216L176 80H40ZM40 104H176L136 144H80V216H40Z'
REVERSE='M41 41H213L175 79H41ZM41 105H173L135 143H79V215H41Z'
SMALL='M32 32H224L176 80H32ZM32 112H192L144 160H80V224H32Z'

def svg(title, body, w=256, h=256):
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w:g} {h:g}" width="{w:g}" height="{h:g}" role="img" aria-labelledby="title"><title id="title">{escape(title)}</title>{body}</svg>\n'

def glyph_word(fontfile, word='fleetboard', spacing=True):
    font=TTFont(fontfile); glyphs=font.getGlyphSet(); cmap=font.getBestCmap()
    pen=SVGPathPen(glyphs); cursor=0; last=0
    # Optical spacing in font units; contours remain unmodified.
    gaps=[160,140,156,160,188,130,148,160,108]
    for i,ch in enumerate(word):
        g=glyphs[cmap[ord(ch)]]; bp=BoundsPen(glyphs); g.draw(bp)
        x0,y0,x1,y1=bp.bounds
        dx=cursor-x0 if spacing else cursor
        g.draw(TransformPen(pen,(1,0,0,1,dx,0)))
        last=dx+x1
        cursor=last+(gaps[i] if i<len(gaps) else 0) if spacing else cursor+g.width
    return pen.getCommands(),last,font['head'].unitsPerEm

def write(path, content):
    path.parent.mkdir(parents=True,exist_ok=True); path.write_text(content)

def render(src, dest, width):
    import xml.etree.ElementTree as ET
    box=list(map(float,ET.parse(src).getroot().get('viewBox').split()))
    cairosvg.svg2png(url=str(src),write_to=str(dest),output_width=width,output_height=round(width*box[3]/box[2]))

def main():
    parser=argparse.ArgumentParser(); parser.add_argument('--font',default='/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf')
    a=parser.parse_args(); word,width,_=glyph_word(a.font)
    scale=.075; wordw=width*scale
    # Type is optically centred against the symbol, with 56 units of visible gap.
    dims={'symbol':(256,256),'horizontal':(round(272+wordw+40,2),256),'stacked':(round(wordw+80,2),440),'wordmark':(round(wordw+80,2),196)}
    for name in dims:
        w,h=dims[name]
        for version in ['color','ink','black','white','orange']:
            symcolor=ORANGE if version=='color' else {'ink':INK,'black':'#000000','white':WHITE,'orange':ORANGE}[version]
            typecolor=INK if version=='color' else symcolor
            mark=REVERSE if version=='white' else MARK
            sym=f'<path fill="{symcolor}" d="{mark}"/>'
            if name=='symbol': body=sym
            elif name=='horizontal': body=sym+f'<path fill="{typecolor}" transform="translate(272 184) scale({scale} -{scale})" d="{word}"/>'
            elif name=='stacked': body=f'<g transform="translate({(w-256)/2:.2f} 0)">{sym}</g><path fill="{typecolor}" transform="translate(40 390) scale({scale} -{scale})" d="{word}"/>'
            else: body=f'<path fill="{typecolor}" transform="translate(40 156) scale({scale} -{scale})" d="{word}"/>'
            path=ROOT/'digital'/f'fleetboard-{name}-{version}.svg'
            write(path,svg(f'fleetboard {name} — {version}',body,w,h))
    for version,colour in [('ink',INK),('white',WHITE),('orange',ORANGE)]:
        write(ROOT/'digital'/f'fleetboard-symbol-small-{version}.svg',svg(f'fleetboard small symbol — {version}',f'<path fill="{colour}" d="{SMALL}"/>'))
    # Small mark has integer pixel edges at 16 px. White is kept on that grid.
    favicon=svg('fleetboard',f'<rect width="256" height="256" rx="48" fill="{ORANGE}"/><path fill="{WHITE}" d="{SMALL}"/>')
    write(ROOT/'web'/'favicon.svg',favicon)
    appbody=f'<rect width="256" height="256" rx="56" fill="{ORANGE}"/><g transform="translate(14 10)"><path fill="{WHITE}" d="{REVERSE}"/></g>'
    # Painted footprint stays below 70% of the tile and is nudged right/down for balance.
    write(ROOT/'digital'/'fleetboard-app-icon.svg',svg('fleetboard app icon',appbody))
    write(ROOT/'digital'/'fleetboard-avatar.svg',svg('fleetboard avatar',f'<rect width="256" height="256" fill="{ORANGE}"/><g transform="translate(14 10)"><path fill="{WHITE}" d="{REVERSE}"/></g>'))
    for size in [16,32,48]: render(ROOT/'web'/'favicon.svg',ROOT/'web'/f'favicon-{size}.png',size)
    images=[Image.open(ROOT/'web'/f'favicon-{s}.png') for s in [16,32,48]]
    images[-1].save(ROOT/'web'/'favicon.ico',format='ICO',sizes=[(16,16),(32,32),(48,48)],append_images=images[:-1])
    # Unrounded full-bleed tiles let the platform apply its own corner mask.
    tile=svg('fleetboard',f'<rect width="256" height="256" fill="{ORANGE}"/><g transform="translate(14 10)"><path fill="{WHITE}" d="{REVERSE}"/></g>')
    for name,size in [('apple-touch-icon',180),('icon-192',192),('icon-512',512)]:
        cairosvg.svg2png(bytestring=tile.encode(),write_to=str(ROOT/'web'/f'{name}.png'),output_width=size,output_height=size)
    # The maskable safe circle has radius 102.4. Artwork is within radius 90.
    mask=svg('fleetboard maskable icon',f'<rect width="256" height="256" fill="{ORANGE}"/><g transform="translate(55 52) scale(.60)"><path fill="{WHITE}" d="{MARK}"/></g>')
    cairosvg.svg2png(bytestring=mask.encode(),write_to=str(ROOT/'web'/'maskable-512.png'),output_width=512,output_height=512)
    manifest={'name':'fleetboard','short_name':'fleetboard','icons':[{'src':f'./{n}.png','sizes':f'{s}x{s}','type':'image/png',**({'purpose':'maskable'} if n=='maskable-512' else {})} for n,s in [('icon-192',192),('icon-512',512),('maskable-512',512)]],'theme_color':ORANGE,'background_color':PAPER,'display':'standalone'}
    write(ROOT/'web'/'site.webmanifest',json.dumps(manifest,indent=2)+'\n')
    write(ROOT/'web'/'head-snippet.html','<!-- Copy web/ assets to the public root, or adjust all paths together. -->\n<link rel="icon" href="/favicon.ico" sizes="any">\n<link rel="icon" href="/favicon.svg" type="image/svg+xml">\n<link rel="apple-touch-icon" href="/apple-touch-icon.png">\n<link rel="manifest" href="/site.webmanifest">\n<meta name="theme-color" content="'+ORANGE+'">\n')
    for src in sorted((ROOT/'digital').glob('*.svg')):
        widths=[1024,2048] if '-horizontal-' in src.name else [512,1024]
        for size in widths: render(src,src.with_name(src.stem+f'-{size}.png'),size)
    for name in ['symbol','horizontal','stacked','wordmark']:
        for version in ['color','black','white']:
            src=ROOT/'digital'/f'fleetboard-{name}-{version}.svg'
            cairosvg.svg2pdf(url=str(src),write_to=str(ROOT/'print'/f'fleetboard-{name}-{version}-rgb.pdf'))
    palette={k:{'hex':c,'rgb':[int(c[i:i+2],16) for i in [1,3,5]]} for k,c in [('ember',ORANGE),('ink',INK),('paper',PAPER),('white',WHITE)]}
    write(ROOT/'source'/'palette.json',json.dumps(palette,indent=2)+'\n')
    write(ROOT/'source'/'geometry.json',json.dumps({'viewBox':[0,0,256,256],'main':MARK,'reversed':REVERSE,'small':SMALL,'wordmark_font':'DejaVu Sans Mono Bold','wordmark_gap_font_units':[160,140,156,160,188,130,148,160,108],'horizontal_gap':56},indent=2)+'\n')
    print(f'Built masters and exports in {ROOT}; horizontal {dims["horizontal"]}; word width {wordw:.2f}')

if __name__=='__main__': main()
