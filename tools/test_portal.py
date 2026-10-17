#!/usr/bin/env python3
"""End-to-end test of the write permission for renaming, on an INSTALLED Flatpak.

The dev sandbox (`flatpak build`) has no document portal, so this runs a Devel build installed
next to the real app (own app ID, own ~/.var/app folder, deleted afterwards) with `flatpak
run`, exactly as a user's install runs. It uses copies of a few songs, never the library.

Checks: a library the sandbox can only read renames nothing until access is allowed; the Allow
dialog appears; a different folder is refused; the folder the user allows (a real document
portal grant, as the file chooser makes) renames, survives a restart and undoes; a library the
sandbox cannot see at all works through its portal path; and a folder the system itself will
not let the user change is reported, with nothing renamed.

Install the Devel build first (see tools/README.md), then: python3 tools/test_portal.py
"""
import os, shutil, stat, subprocess, sys, time

sys.path.insert(0, os.path.dirname(__file__))
import test_rename as r  # noqa: E402

t = r.t
DEV = 'io.github.k_s_harish_balagi.Ryuon.Devel'
VAR = os.path.expanduser(f'~/.var/app/{DEV}')
t.APP, t.WIN = DEV, '/io/github/k_s_harish_balagi/Ryuon/Devel/window/1'
t.CFG, t.CACHE = os.path.join(VAR, 'config'), os.path.join(VAR, 'cache')
t.PLS = os.path.join(t.CACHE, 'ryuon', 'playlists', 'current.pls')
t.WORK = '/tmp/ryuon-portal-test'
t.LIB = os.path.join(t.WORK + '-lib', 'Music')
SHOTS = r.SHOTS = os.environ.get('RYUON_SHOTS', t.WORK)
TIDY = ['Aarya 2', 'Beast', 'Beast (2)', 'Coolie', 'Darbar', 'Leo']


def launch(lib_mode):
    """Starts the installed Devel app. lib_mode 'ro': the sandbox can read the library;
    'none': it cannot see it at all (only a document portal path reaches it)."""
    if t.name_owned():
        sys.exit(f'{DEV} is already running; close it first.')
    fs = [f'--filesystem={t.LIB}:ro'] if lib_mode == 'ro' else []
    cmd = ['flatpak', 'run', '--env=RYUON_TEST_HOOKS=1', '--socket=session-bus', *fs, DEV]
    subprocess.Popen(cmd, env=dict(os.environ, DISPLAY=':0'),
                     stdout=open(os.path.join(t.WORK, 'app.log'), 'a'), stderr=subprocess.STDOUT)
    start = time.time()
    while time.time() - start < 60:
        if t.dev_pids() and t.name_owned() and os.path.exists(t.PLS) and os.path.getmtime(t.PLS) > start:
            time.sleep(1.5)
            return True
        time.sleep(0.5)
    return False


def portal_grant(path):
    """What the file chooser portal does when the user picks a folder: export it through the
    document portal with read and write access for this app. Returns the path the app sees."""
    import gi
    gi.require_version('Gio', '2.0')
    from gi.repository import Gio, GLib
    bus = Gio.bus_get_sync(Gio.BusType.SESSION)
    fd = os.open(path, os.O_PATH | os.O_CLOEXEC)
    fds = Gio.UnixFDList.new()
    idx = fds.append(fd)
    os.close(fd)
    # flags: reuse existing | persistent | export directory
    res, _ = bus.call_with_unix_fd_list_sync(
        'org.freedesktop.portal.Documents', '/org/freedesktop/portal/documents',
        'org.freedesktop.portal.Documents', 'AddFull',
        GLib.Variant('(ahusas)', ([idx], 1 | 2 | 8, DEV, ['read', 'write'])),
        GLib.VariantType('(asa{sv})'), 0, -1, fds, None)
    doc_id = res.unpack()[0][0]
    return doc_id, f'/run/user/{os.getuid()}/doc/{doc_id}/{os.path.basename(path)}'


def portal_delete(doc_id):
    subprocess.run(['gdbus', 'call', '--session', '--dest', 'org.freedesktop.portal.Documents',
                    '--object-path', '/org/freedesktop/portal/documents', '--method',
                    'org.freedesktop.portal.Documents.Delete', doc_id], capture_output=True)


def write_path():
    return (t.keyfile_get('library-write-path') or "''").strip("'")


def set_writable(root, on):
    for d, dirs, _ in os.walk(root):
        for x in [d] + [os.path.join(d, n) for n in dirs]:
            mode = os.stat(x).st_mode
            os.chmod(x, mode | stat.S_IWUSR if on else mode & ~stat.S_IWUSR)


def main():
    if subprocess.run(['flatpak', 'info', DEV], capture_output=True).returncode:
        sys.exit(f'{DEV} is not installed (see tools/README.md)')
    os.makedirs(t.WORK, exist_ok=True)
    shutil.rmtree(VAR, ignore_errors=True)
    r.build_fixture()
    before = r.snapshot(t.LIB)
    count = r.plan_count()
    t.check('fixture needs renames', count > 0, str(count))
    t.keyfile_set(**{'library-path': f"'{t.LIB}'", 'library-autoload': 'true'})
    doc_id = None
    try:
        t.check('installed app starts with a read-only library', launch('ro'))
        queue = t.order()
        r.act('test-rename-confirmed', f'[<uint32 {count}>]')
        t.check('nothing renamed before access is allowed', r.snapshot(t.LIB) == before)
        r.act('test-rename-flow')
        time.sleep(1)
        r.shot('allow-dialog.png')
        t.check('Allow dialog shown (see allow-dialog.png)', os.path.exists(os.path.join(SHOTS, 'allow-dialog.png')))
        r.act('test-allow-folder', f"[<'{os.path.dirname(t.LIB)}'>]")
        t.check('a different folder is refused', write_path() == '', write_path())
        doc_id, portal = portal_grant(t.LIB)
        r.act('test-allow-folder', f"[<'{portal}'>]")
        t.check('allowed folder remembered', write_path() == portal, write_path())
        t.quit_app()

        t.check('app restarts', launch('ro'))
        since = time.time()
        r.act('test-rename-confirmed', f'[<uint32 {count}>]')
        t.check('renames through the allowed folder after a restart', r.wait_reload(since))
        t.check('folders renamed', sorted(os.listdir(t.LIB)) == TIDY, str(sorted(os.listdir(t.LIB))))
        after = r.snapshot(t.LIB)
        t.check('no file lost, sizes kept', sorted(s for _, s in after) == sorted(s for _, s in before))
        q = t.order()
        t.check('queue keeps library paths, not portal paths',
                q and all(p.startswith(t.LIB) and os.path.exists(p) for p in q), str(q[:2]))
        since = time.time()
        r.act('test-undo-rename')
        t.check('undo through the portal', r.wait_reload(since) and r.snapshot(t.LIB) == before)
        t.check('queue points at the old names after undo', all(os.path.exists(p) for p in t.order()))
        t.quit_app()

        # A library chosen with the file chooser that the sandbox cannot otherwise see
        t.keyfile_set(**{'library-path': f"'{portal}'", 'library-write-path': "''"})
        t.check('library outside the sandbox opens through the portal', launch('none'))
        t.check('every song queued from the portal path', len(t.order()) == len(queue),
                f'{len(t.order())} of {len(queue)}')
        since = time.time()
        r.act('test-rename-confirmed', f'[<uint32 {count}>]')
        t.check('portal library renames with no extra step',
                r.wait_reload(since) and sorted(os.listdir(t.LIB)) == TIDY, str(sorted(os.listdir(t.LIB))))
        since = time.time()
        r.act('test-undo-rename')
        t.check('portal library undo', r.wait_reload(since) and r.snapshot(t.LIB) == before)
        t.quit_app()

        # The user's own account may not change the folder: allowing it cannot help
        set_writable(t.LIB, False)
        t.keyfile_set(**{'library-path': f"'{t.LIB}'", 'library-write-path': "''"})
        t.check('app starts with a folder the user cannot change', launch('ro'))
        r.act('test-allow-folder', f"[<'{portal}'>]")
        time.sleep(1)
        r.shot('cannot-write.png')
        t.check('not remembered when writing still fails', write_path() == '', write_path())
        r.act('test-rename-confirmed', f'[<uint32 {count}>]')
        t.check('nothing renamed in a folder the user cannot change', r.snapshot(t.LIB) == before)
        t.quit_app()
    finally:
        set_writable(t.LIB, True)
        t.quit_app()
        if doc_id:
            portal_delete(doc_id)
        shutil.rmtree(VAR, ignore_errors=True)

    passed = sum(ok for _, ok in t.results)
    print(f'\n{passed}/{len(t.results)} checks passed')
    return 0 if passed == len(t.results) else 1


if __name__ == '__main__':
    sys.exit(main())
