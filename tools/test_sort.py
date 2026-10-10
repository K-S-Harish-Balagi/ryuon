#!/usr/bin/env python3
"""End-to-end test of folder sorting and the Folders view (Phase 5), with no input.

Runs the dev build on the real library (read-only), switches the Sort Folders setting over
D-Bus (win.library-sort) and checks the queue order after each change, that the playing
song keeps playing, and that a move switches to Manual. Captures the Folders view and the
Preferences dialog. Usage: python3 tools/test_sort.py
"""
import os, sys, time

sys.path.insert(0, os.path.dirname(__file__))
import test_reorder as t  # noqa: E402

t.WORK = os.environ.get('RYUON_TEST_DIR', '/tmp/ryuon-sort-test')
t.CFG, t.CACHE = os.path.join(t.WORK, 'config'), os.path.join(t.WORK, 'cache')
t.PLS = os.path.join(t.CACHE, 'ryuon', 'playlists', 'current.pls')
SHOTS = os.environ.get('RYUON_SHOTS', t.WORK)
HOOKS = ['--env=RYUON_TEST_HOOKS=1']
ENV = dict(os.environ, DISPLAY=':0')


def act(name, param='[]', wait=1.5):
    t.sh('gdbus', 'call', '--session', '--dest', t.APP, '--object-path', t.WIN,
         '--method', 'org.gtk.Actions.Activate', name, param, '{}')
    time.sleep(wait)


def window():
    w = t.sh('xdotool', 'search', '--onlyvisible', '--class', 'ryuon', env=ENV).stdout.split()
    return w[-1] if w else None


def title():
    w = window()
    return t.sh('xdotool', 'getwindowname', w, env=ENV).stdout.strip() if w else ''


def shot(name):
    if window():
        t.sh('import', '-window', window(), os.path.join(SHOTS, name), env=ENV)


def stable_sort(blocks, key):
    return [b for _, b in sorted(enumerate(blocks), key=lambda ib: (key(ib[1]), ib[0]))]


def main():
    os.makedirs(t.WORK, exist_ok=True)
    t.sh('rm', '-rf', t.CFG, t.CACHE, os.path.join(t.WORK, 'data'))
    t.keyfile_set(**{'library-path': f"'{t.LIB}'", 'library-autoload': 'true', 'resume-uri': "''",
                     'library-sort': "'name'"})
    # Two folders played earlier, for the Recently Played sort
    names = sorted(os.listdir(t.LIB), key=str.lower)
    t.check('app starts', t.launch(HOOKS))
    base = t.order()
    fb = t.blocks(base)
    played = os.path.join(t.WORK, 'data', 'ryuon', 'recent-folders.tsv')
    with open(played, 'a') as f:
        f.write(f'2000\t{os.path.join(t.LIB, fb[10][0])}\n1000\t{os.path.join(t.LIB, fb[20][0])}\n')
    playing = title()

    act('library-sort', '[<"most-songs">]')
    t.check('Most Songs First', t.blocks(t.order()) == stable_sort(fb, lambda b: -b[1]),
            str(t.blocks(t.order())[:5]))
    t.check('the playing song keeps playing', title() == playing, f'{playing!r} -> {title()!r}')

    newest = {}
    for p in base:
        f = t.folder(p)
        newest[f] = max(newest.get(f, 0), int(os.stat(p).st_mtime))
    act('library-sort', '[<"newest">]')
    t.check('Newest First', t.blocks(t.order()) == stable_sort(fb, lambda b: -newest[b[0]]),
            str(t.blocks(t.order())[:5]))

    act('library-sort', '[<"recent">]')
    got = t.blocks(t.order())
    t.check('Recently Played First', got[:3] == [fb[0], fb[10], fb[20]] and
            got[3:] == [b for b in fb if b not in (fb[0], fb[10], fb[20])], str(got[:4]))

    act('library-sort', '[<"name">]')
    t.check('By Name restores the order', t.order() == base)
    t.check('the playing song still plays', title() == playing)

    act('play-next', '[<uint32 30>]')
    t.check('moving a song switches to Manual Order', t.keyfile_get('library-sort') == "'manual'",
            str(t.keyfile_get('library-sort')))
    act('library-sort', '[<"name">]')
    t.check('choosing By Name again undoes the move', t.order() == base)

    act('show-folders')
    shot('folders-view.png')
    act('show-folders')
    act('test-preferences')
    shot('preferences.png')
    t.quit_app()
    t.check('sort setting is kept', t.keyfile_get('library-sort') == "'name'")

    passed = sum(ok for _, ok in t.results)
    print(f'\n{passed}/{len(t.results)} checks passed')
    return 0 if passed == len(t.results) else 1


if __name__ == '__main__':
    sys.exit(main())
