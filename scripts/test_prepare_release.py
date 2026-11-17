"""Run preparation guards against isolated manifests and fake external tools."""
from pathlib import Path
import os
import shutil
import subprocess
import sys
import tempfile
import tomllib

script = Path(__file__).resolve().with_name('prepare-release')
for version, mode in [
    ('4.0.0-alpha.3', 'valid'), ('4.0.0', 'valid'), ('4.0.0+build-test', 'valid'),
    ('4.0.0-alpha.3', 'old-svbump'), ('4.0.0-alpha.3', 'missing-notes'),
    ('4.0.0-alpha.3', 'publish-explicit'), ('4.0.0-alpha.3', 'publish-latest'),
    ('4.0.0-alpha.3', 'wrong-title'), ('4.0.0-alpha.3', 'wrong-prerelease'),
]:
    with tempfile.TemporaryDirectory(prefix='linear-release-qa-') as directory:
        root = Path(directory)
        bin_dir = root / 'bin'
        bin_dir.mkdir()
        manifest = root / 'Cargo.toml'
        original = '[workspace.package]\nversion="3.1.0"\n'
        manifest.write_text(original)
        programs = {
            'changelog': '#!/bin/sh\n' + ('exit 1\n' if mode == 'missing-notes' else 'if [ "$1" = version ]; then echo 4.0.0-alpha.3; fi\n'),
            'cargo': '#!/bin/sh\nprintf "%s\\n" "$*" > cargo-called\n',
            'dist': '#!' + sys.executable + '\n' + '''
import json, os, tomllib
version = tomllib.load(open('Cargo.toml', 'rb'))['workspace']['package']['version']
mode = os.environ['RELEASE_TEST_MODE']
print(json.dumps({
    'announcement_tag': 'v' + version,
    'announcement_title': 'Version ' + version if mode == 'wrong-title' else version + ' - 2026-11-17',
    'announcement_is_prerelease': False if mode == 'wrong-prerelease' else '-' in version.split('+')[0],
    'releases': [{'app_version': version}],
}))
''',
            'svbump': '#!' + sys.executable + '\n' + '''
import os, pathlib, sys, tomllib
if sys.argv[1] == 'write':
    version = '4.0.0' if os.environ['RELEASE_TEST_MODE'] == 'old-svbump' else sys.argv[2]
    pathlib.Path(sys.argv[4]).write_text('[workspace.package]\\nversion=' + repr(version) + '\\n')
else:
    print(tomllib.loads(pathlib.Path(sys.argv[3]).read_text())['workspace']['package']['version'])
''',
        }
        programs['jj'] = '#!/bin/sh\nprintf "jj %s\\n" "$*" >> commands.log\nif [ "$1" = log ]; then printf "%040d" 1; fi\n'
        programs['git'] = '#!/bin/sh\nprintf "git %s\\n" "$*" >> commands.log\n'
        for name, text in programs.items():
            path = bin_dir / name
            path.write_text(text)
            path.chmod(0o755)
        env = dict(os.environ, PATH=str(bin_dir) + os.pathsep + os.environ['PATH'], RELEASE_TEST_MODE=mode)
        publishing = mode.startswith('publish-')
        if publishing:
            (root / 'scripts').mkdir()
            shutil.copy2(script, root / 'scripts/prepare-release')
            command = ['just', '--justfile', str(script.parent.parent / 'justfile'), '--working-directory', str(root), 'tag']
            if mode == 'publish-explicit':
                command.append(version)
        else:
            command = [str(script), version]
        result = subprocess.run(command, cwd=root, env=env, text=True, capture_output=True)
        assert (result.returncode == 0) == (mode == 'valid' or publishing), (mode, result.stdout, result.stderr)
        if mode in ['old-svbump', 'missing-notes']:
            assert manifest.read_text() == original
            assert not (root / 'cargo-called').exists()
        if mode == 'valid' or publishing:
            assert tomllib.loads(manifest.read_text())['workspace']['package']['version'] == version
            if publishing:
                commands = (root / 'commands.log').read_text()
                assert 'jj tag set v' + version + ' -r @-' in commands
                assert commands.splitlines()[-1] == 'git push origin ' + '0' * 39 + '1:refs/tags/v' + version
                assert '--tags' not in commands
            else:
                assert (root / 'cargo-called').read_text().strip() == 'update --workspace'
        print('PASS', version, mode)
