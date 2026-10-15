#!/usr/bin/env python3
"""End-to-end test of Rename Files on Disk (Phase 4b), with no mouse or keyboard input.

Builds a small library from COPIES of a few real songs (the real library is only read),
starts the dev build with RYUON_TEST_HOOKS=1 and drives the test-only actions over D-Bus:
wrong count refused, rename, queue and resume follow, undo restores everything, and a
read-only library shows the permission dialog. Usage: python3 tools/test_rename.py
"""
import os, shutil, subprocess, sys, time, urllib.parse

sys.path.insert(0, os.path.dirname(__file__))
import test_reorder as t  # noqa: E402

SRC_LIB = t.LIB
FOLDERS = ['Leo - MassTamilan.dev', 'Darbar 64Kbps', 'Coolie-320kbps-MassTamilan', 'Beast',
           'Beast - MassTamilan.so', 'Aarya 2']
t.WORK = os.environ.get('RYUON_TEST_DIR', '/tmp/ryuon-rename-test')
t.CFG, t.CACHE = os.path.join(t.WORK, 'config'), os.path.join(t.WORK, 'cache')
t.PLS = os.path.join(t.CACHE, 'ryuon', 'playlists', 'current.pls')
t.LIB = os.path.join(t.WORK + '-lib', 'Music')
HOOKS = ['--env=RYUON_TEST_HOOKS=1']
SHOTS = os.environ.get('RYUON_SHOTS', t.WORK)


def snapshot(root):
    out = []
    for d, dirs, files in os.walk(root):
        for f in files:
            p = os.path.join(d, f)
            out.append((os.path.relpath(p, root), os.path.getsize(p)))
    return sorted(out)


# Where each fixture folder's songs come from now: the developer's real library was itself
# renamed to tidy names on 2026-10-11, so the messy names exist only in the fixture
SOURCES = {'Leo - MassTamilan.dev': 'Leo', 'Darbar 64Kbps': 'Darbar', 'Coolie-320kbps-MassTamilan': 'Coolie',
           'Beast - MassTamilan.so': 'Beast (2)'}


def build_fixture():
    shutil.rmtree(os.path.dirname(t.LIB), ignore_errors=True)
    for folder in FOLDERS:
        src = folder if os.path.isdir(os.path.join(SRC_LIB, folder)) else SOURCES.get(folder, folder)
        songs = sorted(f for f in os.listdir(os.path.join(SRC_LIB, src)) if f.endswith('.mp3'))[:3]
        os.makedirs(os.path.join(t.LIB, folder))
        for s in songs:
            shutil.copy(os.path.join(SRC_LIB, src, s), os.path.join(t.LIB, folder, s))
    open(os.path.join(t.LIB, 'Coolie-320kbps-MassTamilan', 'cover.jpg'), 'w').write('not audio')


def plan_count():
    src = t.SRC
    # same cargo invocation as tools/devbuild.sh test, so the build is reused
    r = subprocess.run(['flatpak', 'build', f'--filesystem={src}', f'--filesystem={os.path.dirname(t.LIB)}',
                        '--env=PATH=/app/rust-toolchain/bin:/app/bin:/usr/bin',
                        '--env=CARGO_PROFILE_RELEASE_OPT_LEVEL=1', '--env=CARGO_PROFILE_RELEASE_LTO=off',
                        '--env=CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16', '--env=CARGO_PROFILE_RELEASE_INCREMENTAL=true',
                        f'--env=CARGO_HOME={src}/.devbuild/cargo-home', f'--env=RYUON_RENAME_DIR={t.LIB}',
                        f'{src}/_dev-build', 'cargo', 'test', '--offline', '--release', '--manifest-path',
                        f'{src}/Cargo.toml', '--target-dir', f'{src}/.devbuild/src', '--', 'print_plan_count',
                        '--ignored', '--nocapture'], capture_output=True, text=True)
    for line in r.stdout.splitlines():
        if line.startswith('RYUON_PLAN_COUNT='):
            return int(line.split('=')[1])
    sys.exit('could not get the plan count:\n' + r.stdout + r.stderr)


def act(name, param='[]'):
    t.sh('gdbus', 'call', '--session', '--dest', t.APP, '--object-path', t.WIN,
         '--method', 'org.gtk.Actions.Activate', name, param, '{}')
    time.sleep(1)


def wait_reload(since):
    for _ in range(60):
        if os.path.exists(t.PLS) and os.path.getmtime(t.PLS) > since:
            time.sleep(1.5)
            return True
        time.sleep(0.5)
    return False


def shot(name):
    env = dict(os.environ, DISPLAY=':0')
    for wid in t.sh('xdotool', 'search', '--onlyvisible', '--class', 'ryuon', env=env).stdout.split():
        t.sh('import', '-window', wid, os.path.join(SHOTS, name), env=env)


def main():
    os.makedirs(t.WORK, exist_ok=True)
    t.sh('rm', '-rf', t.CFG, t.CACHE, os.path.join(t.WORK, 'data'))
    build_fixture()
    before = snapshot(t.LIB)
    count = plan_count()
    t.check('fixture needs renames', count > 0, str(count))
    leo_dir = os.path.join(t.LIB, 'Leo - MassTamilan.dev')
    leo_song = os.path.join(leo_dir, sorted(os.listdir(leo_dir))[0])
    t.keyfile_set(**{'library-path': f"'{t.LIB}'", 'library-autoload': 'true',
                     'resume-uri': f"'{urllib.parse.quote('file://' + leo_song, safe=':/')}'",
                     'resume-position': 'uint64 5'})

    t.check('app starts with the fixture library', t.launch(HOOKS, lib_mode='rw'))
    queue = t.order()

    act('test-rename-confirmed', f'[<uint32 {count + 1}>]')
    t.check('a wrong count renames nothing', snapshot(t.LIB) == before)

    since = time.time()
    act('test-rename-confirmed', f'[<uint32 {count}>]')
    t.check('library reopens after renaming', wait_reload(since))
    after = snapshot(t.LIB)
    dirs = sorted(os.listdir(t.LIB))
    t.check('folders get tidy names', dirs == ['Aarya 2', 'Beast', 'Beast (2)', 'Coolie', 'Darbar', 'Leo'], str(dirs))
    t.check('no file lost, sizes kept', sorted(s for _, s in after) == sorted(s for _, s in before))
    t.check('non-audio files keep their names', os.path.exists(os.path.join(t.LIB, 'Coolie', 'cover.jpg')))
    q = t.order()
    t.check('queue points at the new names', len(q) == len(queue) and all(os.path.exists(p) for p in q))
    journals = os.listdir(os.path.join(t.WORK, 'data', 'ryuon', 'rename-journal'))
    t.check('a journal was written', len(journals) == 1 and journals[0].endswith('.jsonl'), str(journals))
    t.quit_app()
    resume = urllib.parse.unquote(t.keyfile_get('resume-uri') or '')
    t.check('resume point follows the rename', "/Leo/" in resume, resume)

    t.check('app restarts', t.launch(HOOKS, lib_mode='rw'))
    since = time.time()
    act('test-undo-rename')
    t.check('library reopens after undo', wait_reload(since))
    t.check('undo restores every name', snapshot(t.LIB) == before)
    t.check('queue points at the old names', all(os.path.exists(p) for p in t.order()))
    journals = os.listdir(os.path.join(t.WORK, 'data', 'ryuon', 'rename-journal'))
    t.check('journal marked as undone', all(j.endswith('.undone.jsonl') for j in journals), str(journals))
    act('test-rename-preview')
    time.sleep(1)
    shot('rename-preview.png')
    t.quit_app()
    resume = urllib.parse.unquote(t.keyfile_get('resume-uri') or '')
    t.check('resume point follows the undo', '/Leo - MassTamilan.dev/' in resume, resume)

    t.check('app starts with a read-only library', t.launch(HOOKS, lib_mode='ro'))
    act('test-rename-preview')
    time.sleep(1)
    shot('rename-readonly.png')
    t.check('read-only library: nothing renamed', snapshot(t.LIB) == before)
    t.quit_app()

    passed = sum(ok for _, ok in t.results)
    print(f'\n{passed}/{len(t.results)} checks passed')
    return 0 if passed == len(t.results) else 1


if __name__ == '__main__':
    sys.exit(main())
