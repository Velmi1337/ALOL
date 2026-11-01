#!/usr/bin/env python3
"""Exercise updates using an isolated installed executable and fake GitHub transport."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / "target/release/alol"
API = "https://api.github.com/repos/Velmi1337/ALOL/releases/latest"
VERSION = "99.0.0"
ASSET = f"alol-{VERSION}-linux-x86_64"
URL = f"https://github.com/Velmi1337/ALOL/releases/download/v{VERSION}/{ASSET}"


class Updates(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="alol-update-test-")
        self.root = Path(self.temp.name)
        self.installed = self.root / "installed/alol"
        self.installed.parent.mkdir()
        shutil.copyfile(BINARY, self.installed)
        self.installed.chmod(0o755)
        self.tools = self.root / "bin"
        self.tools.mkdir()
        self.trace = self.root / "requests.jsonl"
        self.mapping = self.root / "mapping.json"
        self.env = dict(os.environ, XDG_CONFIG_HOME=str(self.root / "config"),
                        XDG_DATA_HOME=str(self.root / "data"),
                        PATH=str(self.tools) + os.pathsep + os.environ["PATH"],
                        ALOL_UPDATE_FIXTURE=str(self.mapping), ALOL_UPDATE_TRACE=str(self.trace))
        curl = self.tools / "curl"
        curl.write_text("""#!/usr/bin/python3
import json, os, pathlib, shutil, sys, time
a = sys.argv[1:]
with open(os.environ['ALOL_UPDATE_TRACE'], 'a') as f: f.write(json.dumps(a[-1]) + '\\n')
if os.environ.get('ALOL_UPDATE_DELAY'): time.sleep(0.5)
source = json.loads(pathlib.Path(os.environ['ALOL_UPDATE_FIXTURE']).read_text()).get(a[-1])
if source is None: sys.exit(22)
shutil.copyfile(source, a[a.index('--output') + 1])
""")
        curl.chmod(0o755)
        self.payload = self.root / "new-alol"
        subprocess.run(["cc", "-x", "c", "-o", str(self.payload), "-"], input=(
            '#include <stdio.h>\nint main(void) { puts("ALOL 99.0.0"); return 0; }\n'
        ), text=True, check=True, capture_output=True)
        self.release_file = self.root / "release.json"
        self.release = {"tag_name": f"v{VERSION}", "draft": False, "prerelease": False,
                        "assets": [{"name": ASSET, "size": self.payload.stat().st_size,
                                    "digest": "sha256:" + hashlib.sha256(self.payload.read_bytes()).hexdigest()}]}
        self.write_release()
        self.mapping.write_text(json.dumps({API: str(self.release_file), URL: str(self.payload)}))

    def tearDown(self):
        self.temp.cleanup()

    def write_release(self):
        self.release_file.write_text(json.dumps(self.release))

    def run_alol(self, *args, success=True, **extra_env):
        result = subprocess.run([str(self.installed), *args], env=dict(self.env, **extra_env),
                                text=True, capture_output=True, timeout=10)
        self.assertEqual(result.returncode == 0, success, result.stdout + result.stderr)
        return result

    def test_manual_check_reports_new_version_without_installing(self):
        before = self.installed.read_bytes()
        result = self.run_alol("check-update")
        self.assertIn(VERSION, result.stdout)
        self.assertEqual(self.installed.read_bytes(), before)
        self.assertTrue((self.root / "data/alol/updates/release.json").is_file())

    def test_replaces_running_binary_atomically_and_preserves_profiles(self):
        profile = self.root / "config/alol/player.json"
        profile.parent.mkdir(parents=True)
        profile.write_bytes(b"keep my profile")
        self.run_alol("self-update", VERSION)
        self.assertEqual(self.installed.read_bytes(), self.payload.read_bytes())
        self.assertEqual(profile.read_bytes(), b"keep my profile")
        self.assertEqual(self.installed.stat().st_mode & 0o777, 0o755)
        self.assertEqual(self.run_alol("--version").stdout.strip(), f"ALOL {VERSION}")
        self.assertEqual(list(self.installed.parent.glob(".alol-update-*")), [])

    def test_hash_mismatch_keeps_old_binary(self):
        before = self.installed.read_bytes()
        self.release["assets"][0]["digest"] = "sha256:" + "0" * 64
        self.write_release()
        self.run_alol("self-update", success=False)
        self.assertEqual(self.installed.read_bytes(), before)

    def test_missing_asset_and_unsafe_tag_do_not_install(self):
        before = self.installed.read_bytes()
        self.release["assets"][0]["name"] = "../alol"
        self.write_release()
        self.run_alol("self-update", success=False)
        self.release["tag_name"] = "v99.0.0/../bad"
        self.write_release()
        self.run_alol("self-update", success=False)
        self.assertEqual(self.installed.read_bytes(), before)

    def test_offline_or_changed_release_keeps_old_binary(self):
        before = self.installed.read_bytes()
        self.run_alol("self-update", "98.0.0", success=False)
        self.mapping.write_text("{}")
        self.run_alol("self-update", success=False)
        self.assertEqual(self.installed.read_bytes(), before)

    def test_verified_non_elf_payload_is_rejected(self):
        before = self.installed.read_bytes()
        self.payload.write_bytes(b"#!/bin/sh\necho malicious\n")
        self.release["assets"][0].update(size=self.payload.stat().st_size,
                                       digest="sha256:" + hashlib.sha256(self.payload.read_bytes()).hexdigest())
        self.write_release()
        self.run_alol("self-update", success=False)
        self.assertEqual(self.installed.read_bytes(), before)

    def test_wrong_binary_version_does_not_replace_existing_copy(self):
        before = self.installed.read_bytes()
        self.release["tag_name"] = "v99.0.1"
        self.release["assets"][0]["name"] = "alol-99.0.1-linux-x86_64"
        self.write_release()
        newer_url = "https://github.com/Velmi1337/ALOL/releases/download/v99.0.1/alol-99.0.1-linux-x86_64"
        self.mapping.write_text(json.dumps({API: str(self.release_file), newer_url: str(self.payload)}))
        self.run_alol("self-update", success=False)
        self.assertEqual(self.installed.read_bytes(), before)

    def package_fixture(self, fedora=False):
        package_name = f"alol-{VERSION}-1.fc44.x86_64.rpm" if fedora else f"alol-{VERSION}-1-x86_64.pkg.tar.zst"
        self.release["assets"][0]["name"] = package_name
        self.write_release()
        package_url = f"https://github.com/Velmi1337/ALOL/releases/download/v{VERSION}/{package_name}"
        self.mapping.write_text(json.dumps({API: str(self.release_file), package_url: str(self.payload)}))
        owner = self.tools / ("rpm" if fedora else "pacman")
        owner.write_text("#!/bin/sh\nprintf 'alol\\n'\n")
        owner.chmod(0o755)
        pkexec = self.tools / "pkexec"
        pkexec.write_text("""#!/usr/bin/python3
import json, os, pathlib, shutil, sys
root = pathlib.Path(os.environ['ALOL_UPDATE_FIXTURE']).parent
(root / 'pkexec.json').write_text(json.dumps(sys.argv[1:]))
if os.environ.get('ALOL_UPDATE_CANCEL'): sys.exit(126)
temporary = root / 'installed/replacement'
shutil.copyfile(root / 'new-alol', temporary)
temporary.chmod(0o755)
os.replace(temporary, root / 'installed/alol')
""")
        pkexec.chmod(0o755)

    @unittest.skipUnless(Path("/usr/bin/pacman").is_file(), "Arch package manager test")
    def test_package_install_uses_system_manager_and_checks_result(self):
        self.package_fixture()
        self.run_alol("self-update")
        args = json.loads((self.root / "pkexec.json").read_text())
        self.assertEqual(args[:4], ["/usr/bin/pacman", "-U", "--noconfirm", "--"])
        self.assertTrue(args[4].endswith(f"alol-{VERSION}-1-x86_64.pkg.tar.zst"))
        self.assertEqual(self.installed.read_bytes(), self.payload.read_bytes())

    @unittest.skipUnless(Path("/usr/bin/pacman").is_file(), "Arch package manager test")
    def test_cancelled_package_authorization_preserves_installed_binary(self):
        self.package_fixture()
        before = self.installed.read_bytes()
        self.run_alol("self-update", success=False, ALOL_UPDATE_CANCEL="1")
        self.assertEqual(self.installed.read_bytes(), before)

    @unittest.skipUnless(Path("/usr/bin/dnf").is_file(), "Fedora package manager test")
    def test_fedora_package_uses_dnf_and_checks_result(self):
        self.package_fixture(fedora=True)
        self.run_alol("self-update")
        args = json.loads((self.root / "pkexec.json").read_text())
        self.assertEqual(args[:4], ["/usr/bin/dnf", "install", "--assumeyes", "--"])
        self.assertTrue(args[4].endswith(f"alol-{VERSION}-1.fc44.x86_64.rpm"))
        self.assertEqual(self.installed.read_bytes(), self.payload.read_bytes())

    def test_older_release_never_downgrades(self):
        before = self.installed.read_bytes()
        self.release["tag_name"] = "v0.0.1"
        self.write_release()
        result = self.run_alol("self-update")
        self.assertIn("актуальная версия", result.stdout)
        self.assertEqual(self.installed.read_bytes(), before)

    def test_parallel_updates_are_locked(self):
        child = subprocess.Popen([str(self.installed), "self-update"], env=dict(self.env, ALOL_UPDATE_DELAY="1"),
                                 stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        try:
            deadline = time.monotonic() + 3
            while not self.trace.exists() and time.monotonic() < deadline:
                time.sleep(0.01)
            self.assertTrue(self.trace.exists())
            self.assertIn("другом окне", self.run_alol("self-update", success=False).stderr)
            stdout, stderr = child.communicate(timeout=10)
            self.assertEqual(child.returncode, 0, stdout + stderr)
        finally:
            if child.poll() is None:
                child.kill()
                child.communicate()


if __name__ == "__main__":
    unittest.main(verbosity=2)
