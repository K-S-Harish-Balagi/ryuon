#!/usr/bin/env python3
"""Makes a small demo music library for screenshots and trying Ryuon: invented albums with
generated tones and cover art drawn here, so nothing in it is copyrighted. Some folder and
file names are deliberately messy, to show Tidy Names. Needs ffmpeg and ImageMagick.
Usage: python3 tools/make_demo_library.py OUTPUT_DIR
"""
import os, subprocess, sys

ALBUMS = [
    # folder name on disk, album, artist, cover colours, song titles
    ('Monsoon_Evenings-320kbps', 'Monsoon Evenings', 'Ryuon Demo Ensemble', ('#0b3d91', '#5fb3ff'),
     ['First Rain', 'Clay Roofs', 'Paper Boats', 'Petrichor', 'Lamps at Dusk']),
    ('Harbour Lights - DemoSite.in', 'Harbour Lights', 'The Lantern Quartet', ('#063b4c', '#2ec4b6'),
     ['Low Tide', 'Ferry at Nine', 'Gulls', 'Salt and Rope']),
    ('Morning Ragas (2026)', 'Morning Ragas', 'Asha Varma', ('#7a2e0e', '#ffb347'),
     ['Bhoopali Sketch', 'Sunrise on the Ghats', 'Tea Steam', 'River Bells', 'Kites', 'Long Shadows']),
    ('Night-Train-to-Madurai', 'Night Train to Madurai', 'Coastal Drift', ('#1d1b4b', '#9b8cff'),
     ['Platform Four', 'Sleeper Class', 'Signal Lamps']),
    ('Blue Hour 64Kbps', 'Blue Hour', 'Mira Sen', ('#102a43', '#829ab1'),
     ['Window Seat', 'Quiet Street', 'Last Bus Home', 'Neon Puddles']),
]
NOTES = [261.63, 293.66, 329.63, 392.0, 440.0, 523.25]


def run(*cmd):
    subprocess.run(cmd, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def main(out):
    for a, (folder, album, artist, (c1, c2), songs) in enumerate(ALBUMS):
        d = os.path.join(out, folder)
        os.makedirs(d, exist_ok=True)
        cover = os.path.join(d, '.cover.png')
        # gradient with a soft circle and the album initial
        run('convert', '-size', '600x600', f'gradient:{c2}-{c1}', '-fill', 'rgba(255,255,255,0.18)',
            '-draw', 'circle 300,300 300,120', '-fill', 'white', '-font', 'DejaVu-Sans-Bold',
            '-pointsize', '200', '-gravity', 'center', '-annotate', '+0+0', album[0], cover)
        for i, title in enumerate(songs):
            f1, f2 = NOTES[(a + i) % 6], NOTES[(a + 2 * i + 1) % 6]
            messy = f'{i + 1:02d}-{title.replace(" ", "-")}-DemoSite.in.mp3' if a % 2 == 0 else f'{i + 1:02d} {title}.mp3'
            run('ffmpeg', '-y', '-f', 'lavfi', '-i', f'sine=frequency={f1}:duration=40', '-f', 'lavfi',
                '-i', f'sine=frequency={f2}:duration=40', '-i', cover, '-filter_complex',
                '[0][1]amix=inputs=2,volume=0.3,afade=t=in:d=2,afade=t=out:st=37:d=3[a]',
                '-map', '[a]', '-map', '2', '-c:a', 'libmp3lame', '-b:a', '96k', '-c:v', 'png',
                '-id3v2_version', '3', '-metadata', f'title={title}', '-metadata', f'artist={artist}',
                '-metadata', f'album={album}', '-metadata', f'track={i + 1}',
                '-metadata:s:v', 'title=Album cover', '-metadata:s:v', 'comment=Cover (front)',
                os.path.join(d, messy))
        os.remove(cover)
    print('demo library in', out)


if __name__ == '__main__':
    main(sys.argv[1] if len(sys.argv) > 1 else 'demo-library')
