#!/usr/bin/env python3
"""Takes the README and metainfo screenshots from the release build (_flatpak-build), using throwaway settings
and the demo library (tools/make_demo_library.py), with no mouse or keyboard input.
Usage: python3 tools/screenshots.py DEMO_LIBRARY OUTPUT_DIR   (X11 session; needs xdotool,
wmctrl and ImageMagick)
"""
import os, subprocess, sys, tempfile, time

APP = 'io.github.k_s_harish_balagi.Ryuon'
WIN = '/io/github/k_s_harish_balagi/Ryuon/window/1'
SHOWN = '/var/home/demo/Music'  # where the library appears inside the sandbox (and in Preferences)
ENV = dict(os.environ, DISPLAY=os.environ.get('DISPLAY', ':0'))


def sh(*a):
    return subprocess.run(a, capture_output=True, text=True, env=ENV)


def owned():
    r = sh('gdbus', 'call', '--session', '--dest', 'org.freedesktop.DBus', '--object-path',
           '/org/freedesktop/DBus', '--method', 'org.freedesktop.DBus.NameHasOwner', APP)
    return 'true' in r.stdout


def act(name, param='[]', path=WIN, wait=1.5):
    sh('gdbus', 'call', '--session', '--dest', APP, '--object-path', path,
       '--method', 'org.gtk.Actions.Activate', name, param, '{}')
    time.sleep(wait)


def window():
    w = sh('xdotool', 'search', '--onlyvisible', '--class', 'ryuon').stdout.split()
    return w[-1] if w else None


def shot(path):
    raw = path + '.raw.png'
    sh('import', '-window', window(), raw)
    # drop the client-side shadow (25 px) and give the window transparent rounded corners
    w, h = (int(v) - 50 for v in sh('identify', '-format', '%w %h', raw).stdout.split())
    sh('convert', raw, '-shave', '25x25', '(', '-size', f'{w}x{h}', 'xc:black', '-fill', 'white',
       '-draw', f'roundrectangle 0,0 {w - 1},{h - 1} 12,12', ')',
       '-alpha', 'off', '-compose', 'CopyOpacity', '-composite', path)
    os.remove(raw)


def main(library, out):
    if owned():
        sys.exit('Close Ryuon first.')
    os.makedirs(out, exist_ok=True)
    tmp = tempfile.mkdtemp(prefix='ryuon-shots-')
    cfg = os.path.join(tmp, 'config', 'glib-2.0', 'settings')
    os.makedirs(cfg)
    with open(os.path.join(cfg, 'keyfile'), 'w') as f:
        f.write(f"[io/github/k_s_harish_balagi/Ryuon]\nlibrary-path='{SHOWN}'\nlibrary-autoload=true\n"
                "window-width=900\nwindow-height=640\n")
    # `flatpak run` would force the user's own config folders; run the release build folder
    # instead, where the throwaway folders apply
    build = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), '_flatpak-build')
    subprocess.Popen(['flatpak', 'build', '--socket=wayland', '--socket=fallback-x11', '--device=dri',
                      '--share=ipc', '--socket=session-bus', f'--bind-mount={SHOWN}={library}',
                      f'--filesystem={tmp}', '--env=GSETTINGS_BACKEND=keyfile',
                      f'--env=XDG_CONFIG_HOME={tmp}/config', f'--env=XDG_CACHE_HOME={tmp}/cache',
                      f'--env=XDG_DATA_HOME={tmp}/data', '--env=RYUON_TEST_HOOKS=1', build, 'ryuon'],
                     env=ENV, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    for _ in range(60):
        if owned() and window():
            break
        time.sleep(0.5)
    time.sleep(5)
    sh('wmctrl', '-ir', window(), '-e', '0,-1,-1,900,640')
    time.sleep(2)
    shot(os.path.join(out, 'playing.png'))
    act('show-folders', wait=2)
    shot(os.path.join(out, 'folders.png'))
    act('show-folders')
    act('test-preferences', wait=2)
    shot(os.path.join(out, 'preferences.png'))
    act('quit', path='/io/github/k_s_harish_balagi/Ryuon')
    print('screenshots in', out)


if __name__ == '__main__':
    main(sys.argv[1], sys.argv[2])
