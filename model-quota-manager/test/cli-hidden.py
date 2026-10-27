"""macOS PTY integration: configure must never echo a credential, even on immediate paste."""
import json
import os
import pathlib
import pty
import select
import shutil
import subprocess
import tempfile
import time

root = pathlib.Path(__file__).resolve().parent.parent
for _ in range(5):
    folder = tempfile.mkdtemp(prefix='quota-cli-check-')
    master, slave = pty.openpty()
    process = subprocess.Popen(
        ['node', 'configure.mjs', 'deepseek'], cwd=root,
        stdin=slave, stdout=slave, stderr=slave,
        env=dict(os.environ, QUOTA_DATA_DIR=folder), start_new_session=True,
    )
    os.close(slave)
    try:
        output, sent, deadline = b'', False, time.time() + 8
        while time.time() < deadline:
            ready, _, _ = select.select([master], [], [], .1)
            if ready:
                try:
                    output += os.read(master, 8192)
                except OSError:
                    break
            if not sent and '密钥：'.encode() in output:
                os.write(master, b'fixture-cli-hidden-key\r')
                sent = True
            if process.poll() is not None:
                break
        process.wait(timeout=2)
        assert process.returncode == 0 and sent, 'interactive configuration failed'
        assert b'fixture-cli-hidden-key' not in output, 'credential echo detected'
        file = pathlib.Path(folder) / 'credentials.json'
        assert json.loads(file.read_text())['deepseek']['key'] == 'fixture-cli-hidden-key'
        assert file.stat().st_mode & 0o777 == 0o600
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()
        os.close(master)
        shutil.rmtree(folder)
print('PASS: 5 immediate-paste PTY checks; no credential echo; local file mode 0600.')
