#!/usr/bin/env python3
"""End-to-end test of queue reordering (Phase 3), with no mouse or keyboard input.

Launches the dev build (tools/devbuild.sh output in _dev-build) with throwaway settings and
cache, triggers window actions over D-Bus (org.gtk.Actions), and checks the queue order in
the saved playlist file after each step. Needs a library of at least ~100 songs in
several folders: RYUON_TEST_LIBRARY=/path/to/Music python3 tools/test_reorder.py
"""
import configparser, os, subprocess, sys, time, urllib.parse

SRC = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
WORK = os.environ.get('RYUON_TEST_DIR', '/tmp/ryuon-reorder-test')
CFG, CACHE = os.path.join(WORK, 'config'), os.path.join(WORK, 'cache')
APP = 'io.github.k_s_harish_balagi.Ryuon'
WIN = '/io/github/k_s_harish_balagi/Ryuon/window/1'
PLS = os.path.join(CACHE, 'ryuon', 'playlists', 'current.pls')
LIB = os.environ.get('RYUON_TEST_LIBRARY', os.path.expanduser('~/Music'))
results = []


def sh(*a, **k):
    return subprocess.run(a, capture_output=True, text=True, **k)


def keyfile_set(**kv):
    path = os.path.join(CFG, 'glib-2.0', 'settings', 'keyfile')
    os.makedirs(os.path.dirname(path), exist_ok=True)
    lines = [l for l in (open(path).read().splitlines() if os.path.exists(path) else []) if l.strip()]
    group = '[io/github/k_s_harish_balagi/Ryuon]'
    if group not in lines:
        lines.insert(0, group)
    for k, v in kv.items():
        lines = [l for l in lines if not l.startswith(k + '=')]
        lines.append(f'{k}={v}')
    open(path, 'w').write('\n'.join(lines) + '\n')


def keyfile_get(key):
    path = os.path.join(CFG, 'glib-2.0', 'settings', 'keyfile')
    for l in open(path).read().splitlines():
        if l.startswith(key + '='):
            return l.split('=', 1)[1]
    return None


def dev_pids():
    """Ryuon processes started by this test (identified by the throwaway config dir)."""
    pids = []
    for pid in sh('pgrep', '-x', 'ryuon').stdout.split():
        try:
            if f'XDG_CONFIG_HOME={CFG}'.encode() in open(f'/proc/{pid}/environ', 'rb').read():
                pids.append(pid)
        except OSError:
            pass
    return pids


def name_owned():
    # Never call the app name blindly: D-Bus activation would start the installed Flatpak.
    r = sh('gdbus', 'call', '--session', '--dest', 'org.freedesktop.DBus', '--object-path',
           '/org/freedesktop/DBus', '--method', 'org.freedesktop.DBus.NameHasOwner', APP)
    return 'true' in r.stdout


def launch(extra=(), lib_mode='ro'):
    if name_owned():
        sys.exit('Another Ryuon instance owns the bus name; close it first.')
    env = dict(os.environ, DISPLAY=':0')
    lib = f'--filesystem={LIB}:ro' if lib_mode == 'ro' else f'--filesystem={LIB}'
    cmd = ['flatpak', 'build', '--socket=session-bus', '--socket=wayland', '--socket=fallback-x11',
           '--socket=pulseaudio', '--device=dri', '--share=ipc', f'--filesystem={WORK}', lib,
           '--env=GSETTINGS_BACKEND=keyfile', f'--env=XDG_CONFIG_HOME={CFG}',
           f'--env=XDG_CACHE_HOME={CACHE}', f'--env=XDG_DATA_HOME={os.path.join(WORK, "data")}',
           *extra, f'{SRC}/_dev-build', 'ryuon']
    subprocess.Popen(cmd, env=env, stdout=open(os.path.join(WORK, 'app.log'), 'a'), stderr=subprocess.STDOUT)
    # wait until the library is queued: the playlist file is rewritten after loading
    start = time.time()
    while time.time() - start < 60:
        if dev_pids() and name_owned() and os.path.exists(PLS) and os.path.getmtime(PLS) > start:
            time.sleep(1.5)
            return True
        time.sleep(0.5)
    return False


def quit_app():
    # app.quit over D-Bus (the same path as Ctrl+Q); launch() made sure the name is ours
    if dev_pids() and name_owned():
        sh('gdbus', 'call', '--session', '--dest', APP, '--object-path',
           '/io/github/k_s_harish_balagi/Ryuon', '--method', 'org.gtk.Actions.Activate', 'quit', '[]', '{}')
    for _ in range(20):
        if not dev_pids():
            return
        time.sleep(0.5)
    for pid in dev_pids():
        sh('kill', pid)


def act(name, pos=None):
    if not dev_pids() or not name_owned():
        return False
    param = f'[<uint32 {pos}>]' if pos is not None else '[]'
    r = sh('gdbus', 'call', '--session', '--dest', APP, '--object-path', WIN,
           '--method', 'org.gtk.Actions.Activate', name, param, '{}')
    time.sleep(0.8)
    return r.returncode == 0


def order():
    c = configparser.RawConfigParser(strict=False)
    c.optionxform = str
    c.read(PLS)
    n = int(c['playlist']['NumberOfEntries'])
    return [c['playlist'][f'File{i}'] for i in range(n)]


def folder(path):
    return os.path.relpath(path, LIB).split('/')[0]


def blocks(lst):
    out = []
    for p in lst:
        f = folder(p)
        if not out or out[-1][0] != f:
            out.append([f, 0])
        out[-1][1] += 1
    return [tuple(b) for b in out]


def check(name, ok, detail=''):
    results.append((name, bool(ok)))
    print(('PASS ' if ok else 'FAIL ') + name + (f'  [{detail}]' if detail and not ok else ''))


def main():
    os.makedirs(WORK, exist_ok=True)
    sh('rm', '-rf', CFG, CACHE)
    keyfile_set(**{'library-path': f"'{LIB}'", 'library-autoload': 'true', 'resume-uri': "''",
                   'library-sort': "'name'"})
    quit_app()
    check('app starts and loads the library', launch())
    base = order()
    check('queue has all songs', len(base) >= 455, f'{len(base)} songs')
    fb = blocks(base)

    act('play-next', 9)
    o = order()
    check('play-next puts song 9 right after the current song', o == [base[0], base[9]] + base[1:9] + base[10:])
    act('undo-move')
    check('undo restores the order', order() == base)

    s4 = sum(n for _, n in fb[:4])
    act('folder-next', s4)
    exp = [fb[0], fb[4], fb[1], fb[2], fb[3]] + fb[5:]
    check('folder-next moves folder 5 right after the current folder', blocks(order()) == exp,
          str(blocks(order())[:6]))
    act('undo-move')
    check('undo restores after a folder move', order() == base)

    s1 = fb[0][1]
    act('folder-down', s1)
    check('folder-down swaps folders 2 and 3', blocks(order())[:4] == [fb[0], fb[2], fb[1], fb[3]])
    act('folder-up', s1 + fb[2][1])
    check('folder-up moves it back', order() == base)

    act('move-down', 3)
    check('move-down swaps songs 4 and 5', order()[3:5] == [base[4], base[3]])
    act('move-up', 4)
    check('move-up moves it back', order() == base)

    act('play-next', 20)
    moved = order()
    check('manual order is remembered in settings', keyfile_get('library-sort') == "'manual'")
    quit_app()
    check('app restarts', launch())
    check('manual order survives a restart', order() == moved)
    act('remove-at', 5)
    check('remove-at removes one song', order() == moved[:5] + moved[6:])
    act('reset-order')
    time.sleep(8)
    check('reset to folder order restores disk order', order() == base)
    quit_app()

    passed = sum(ok for _, ok in results)
    print(f'\n{passed}/{len(results)} checks passed')
    return 0 if passed == len(results) else 1


if __name__ == '__main__':
    sys.exit(main())
