#!/usr/bin/env python3
"""Exercise the real CLI, native framing and Nautilus helper against a local peer.

No model, user configuration, clipboard, or working desktop is accessed.
"""
import json
import os
from pathlib import Path
import socket
import struct
import subprocess
import sys
import tempfile
import threading

binary = Path(sys.argv[1] if len(sys.argv) > 1 else 'target/release/lexington').resolve()
root = Path(__file__).resolve().parent.parent
with tempfile.TemporaryDirectory(prefix='lexington-context-smoke-') as temporary:
    directory = Path(temporary)
    runtime = directory / 'runtime'
    inbox = runtime / f'lexington-{os.geteuid()}'
    inbox.mkdir(parents=True, mode=0o700)
    data = directory / 'data'
    (data / 'lexington').mkdir(parents=True)
    (data / 'lexington/integrations.json').write_text(json.dumps({'executable': str(binary)}))
    environment = dict(os.environ, XDG_RUNTIME_DIR=str(runtime), XDG_DATA_HOME=str(data))
    captured = []
    failures = []
    stopping = threading.Event()
    listener = socket.socket(socket.AF_UNIX)
    listener.bind(str(inbox / 'inbox.sock'))
    listener.listen()
    listener.settimeout(0.2)

    def serve():
        while not stopping.is_set():
            try:
                stream, _ = listener.accept()
            except socket.timeout:
                continue
            try:
                with stream, stream.makefile('rb') as incoming:
                    stream.settimeout(3)
                    length = struct.unpack('<I', incoming.read(4))[0]
                    captured.append(json.loads(incoming.read(length)))
                    ack = b'{"ok":true}'
                    stream.sendall(struct.pack('<I', len(ack)) + ack)
            except Exception as error:
                failures.append(error)

    worker = threading.Thread(target=serve)
    worker.start()
    try:
        file = directory / '中文 " $(不执行)\n空 格.rs'
        file.write_text('selected file fixture')
        request = {'text': '第一行\n第二行', 'files': [str(file)], 'source': 'VS Code', 'line': 4}
        result = subprocess.run([str(binary), '--ask-json'], input=json.dumps(request).encode(), env=environment, capture_output=True, timeout=5)
        assert result.returncode == 0, result.stderr.decode()
        subprocess.run([str(binary), '--ask-files', str(file)], env=environment, check=True, capture_output=True, timeout=5)
        encoded = json.dumps(request).encode()
        result = subprocess.run([str(binary), '--native-messaging'], input=struct.pack('<I', len(encoded)) + encoded, env=environment, capture_output=True, timeout=5)
        assert result.returncode == 0, result.stderr.decode()
        length = struct.unpack('<I', result.stdout[:4])[0]
        assert len(result.stdout) == length + 4
        assert json.loads(result.stdout[4:]) == {'ok': True}
        environment['NAUTILUS_SCRIPT_SELECTED_URIS'] = file.as_uri() + '\n'
        subprocess.run([sys.executable, str(root / 'integrations/nautilus/send.py')], env=environment, check=True, capture_output=True, timeout=5)
        assert len(captured) == 4 and not failures, failures
        assert captured[0] == request and captured[2] == request
        assert captured[1]['files'] == captured[3]['files'] == [str(file)]
        for invalid in (b'{}', b'{"text":"x","files":["relative"]}', b'{"text":"x","extra":"bad"}', b'x' * (1024 * 1024 + 1)):
            result = subprocess.run([str(binary), '--ask-json'], input=invalid, env=environment, capture_output=True, timeout=5)
            assert result.returncode != 0
        assert len(captured) == 4, 'invalid requests must not reach the queue'
        invalid = b'{}'
        result = subprocess.run([str(binary), '--native-messaging'], input=struct.pack('<I', len(invalid)) + invalid, env=environment, capture_output=True, timeout=5)
        assert not json.loads(result.stdout[4:])['ok']
        print('CLI, browser native framing, Nautilus URI decoding and rejected-input checks passed.')
    finally:
        stopping.set()
        worker.join(timeout=5)
        listener.close()
