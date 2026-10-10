#!/usr/bin/env python3
"""Load-time check (Phase 6): time to window and time until the whole library is queued,
for the real library and for a synthetic one of 200 folders x 10 songs (2-second silent
MP3s made with ffmpeg in a scratch folder). Usage: python3 tools/test_perf.py
"""
import os, shutil, subprocess, sys, time

sys.path.insert(0, os.path.dirname(__file__))
import test_reorder as t  # noqa: E402

BASE = os.environ.get('RYUON_TEST_DIR', '/tmp/ryuon-perf-test')
ENV = dict(os.environ, DISPLAY=':0')


def synthetic(root, folders=200, songs=10):
    if os.path.isdir(root) and sum(len(f) for _, _, f in os.walk(root)) == folders * songs:
        return
    shutil.rmtree(root, ignore_errors=True)
    os.makedirs(os.path.dirname(root), exist_ok=True)
    seed = os.path.join(os.path.dirname(root), 'seed.mp3')
    subprocess.run(['ffmpeg', '-loglevel', 'error', '-y', '-f', 'lavfi', '-i', 'anullsrc=r=44100:cl=stereo',
                    '-t', '2', '-b:a', '64k', seed], check=True)
    for f in range(folders):
        d = os.path.join(root, f'Album {f:03d}-128kbps-MassTamilan.dev')
        os.makedirs(d)
        for s in range(songs):
            shutil.copy(seed, os.path.join(d, f'{s + 1:02d}-Track-{s + 1}-MassTamilan.dev.mp3'))


def measure(lib, label):
    t.WORK = os.path.join(BASE, label)
    t.CFG, t.CACHE = os.path.join(t.WORK, 'config'), os.path.join(t.WORK, 'cache')
    t.PLS = os.path.join(t.CACHE, 'ryuon', 'playlists', 'current.pls')
    t.LIB = lib
    os.makedirs(t.WORK, exist_ok=True)
    # Keep GStreamer's registry and the font and shader caches (a real install keeps them);
    # drop only Ryuon's own saved playlist and settings
    t.sh('rm', '-rf', t.CFG, os.path.join(t.CACHE, 'ryuon'))
    t.keyfile_set(**{'library-path': f"'{lib}'", 'library-autoload': 'true', 'resume-uri': "''"})
    for _ in range(40):  # the previous window must be gone before timing
        if not t.sh('xdotool', 'search', '--onlyvisible', '--class', 'ryuon', env=ENV).stdout.strip():
            break
        time.sleep(0.25)
    start = time.time()
    window_at = None
    ok = False
    # launch() waits for the playlist file; watch for the window in the meantime
    import threading

    def watch():
        nonlocal window_at
        while window_at is None and time.time() - start < 60:
            if t.sh('xdotool', 'search', '--onlyvisible', '--class', 'ryuon', env=ENV).stdout.strip():
                window_at = time.time() - start
            time.sleep(0.05)
    th = threading.Thread(target=watch)
    th.start()
    ok = t.launch()
    queued_at = time.time() - start - 1.5  # launch() sleeps 1.5 s after the file appears
    th.join()
    n = len(t.order()) if ok else 0
    rss = 0
    for pid in t.dev_pids():
        for line in open(f'/proc/{pid}/status'):
            if line.startswith('VmRSS'):
                rss = int(line.split()[1]) // 1024
    t.quit_app()
    print(f'{label}: {n} songs; window after {window_at or -1:.1f} s; all queued after {queued_at:.1f} s; '
          f'memory {rss} MB')
    return ok, n, window_at, queued_at


def main():
    synthetic_root = os.path.join(BASE, 'synthetic', 'Music')
    synthetic(synthetic_root)
    measure(t.LIB, 'real')  # warm-up: builds the caches once
    r1 = measure(t.LIB, 'real')
    measure(synthetic_root, 'synthetic')
    r2 = measure(synthetic_root, 'synthetic')
    t.check('real library: window within 2 s', r1[2] is not None and r1[2] < 2, str(r1[2]))
    t.check('real library: all songs queued', r1[1] >= 455, str(r1[1]))
    t.check('2000 songs: window within 2 s', r2[2] is not None and r2[2] < 2, str(r2[2]))
    t.check('2000 songs: all queued', r2[1] == 2000, str(r2[1]))
    passed = sum(ok for _, ok in t.results)
    print(f'\n{passed}/{len(t.results)} checks passed')
    return 0 if passed == len(t.results) else 1


if __name__ == '__main__':
    sys.exit(main())
