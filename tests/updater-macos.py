"""Exercise Tauri's real HTTPS, signature and macOS bundle replacement with synthetic data."""
import http.server
import json
import os
from pathlib import Path
import ssl
import subprocess
import sys
import tarfile
import tempfile
import threading

if sys.platform != 'darwin':
    sys.exit('Run this test on macOS.')
repo = Path(__file__).resolve().parents[1]
tauri = repo / 'node_modules/.bin/tauri'


def quiet(*args):
    result = subprocess.run(args, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    if result.returncode:
        raise RuntimeError(f'{Path(args[0]).name} failed with code {result.returncode}')


with tempfile.TemporaryDirectory(prefix='starfolio-updater-test-') as directory:
    root = Path(directory)
    signing = root / 'updater.key'
    old = root / 'Installed.app/Contents/MacOS/fixture'
    new = root / 'Fixture.app/Contents/MacOS/fixture'
    for path, content in [(old, b'old'), (new, b'new')]:
        path.parent.mkdir(parents=True)
        path.write_bytes(content)
    package = root / 'Fixture.app.tar.gz'
    with tarfile.open(package, 'w:gz') as archive:
        archive.add(root / 'Fixture.app', arcname='Fixture.app')
    quiet(tauri, 'signer', 'generate', '--ci', '-w', signing)
    quiet(tauri, 'signer', 'sign', '-f', signing, '-p', '', '--app-version', '0.2.3', package)
    signature = (root / 'Fixture.app.tar.gz.sig').read_text()
    payload = package.read_bytes()

    # Trust this test CA only inside the Rust test client. Do not change the Keychain.
    ca_key, ca, key, csr, cert = (root / name for name in ['ca.key','ca.crt','tls.key','tls.csr','tls.crt'])
    ca_conf, leaf_conf = root / 'ca.cnf', root / 'leaf.cnf'
    ca_conf.write_text('[req]\nprompt=no\ndistinguished_name=dn\nx509_extensions=ca\n[dn]\nCN=StarFolio fixture CA\n[ca]\nbasicConstraints=critical,CA:TRUE\nkeyUsage=critical,keyCertSign\n')
    leaf_conf.write_text('basicConstraints=critical,CA:FALSE\nextendedKeyUsage=serverAuth\nsubjectAltName=IP:127.0.0.1\n')
    quiet('openssl','req','-x509','-newkey','rsa:2048','-nodes','-days','1','-config',ca_conf,'-keyout',ca_key,'-out',ca)
    quiet('openssl','req','-new','-newkey','rsa:2048','-nodes','-subj','/CN=127.0.0.1','-keyout',key,'-out',csr)
    quiet('openssl','x509','-req','-in',csr,'-CA',ca,'-CAkey',ca_key,'-CAcreateserial','-days','1','-extfile',leaf_conf,'-out',cert)

    class Handler(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            versions = {'/valid':'0.2.3','/wrong-version':'0.2.4','/tampered':'0.2.3','/current':'0.2.2','/older':'0.2.1'}
            code, body, mime = 200, b'', 'application/octet-stream'
            if self.path in versions:
                path = '/bad-package' if self.path == '/tampered' else '/package'
                platform = {'url': endpoint + path, 'signature': signature}
                body = json.dumps({'version':versions[self.path],'platforms':{'darwin-aarch64':platform,'darwin-x86_64':platform}}).encode()
                mime = 'application/json'
            elif self.path == '/package':
                body = payload
            elif self.path == '/bad-package':
                body = payload + b'tampered'
            else:
                code = 204 if self.path == '/empty' else 404
            self.send_response(code)
            self.send_header('Content-Type', mime)
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)

    with http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler) as server:
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(cert, key)
        server.socket = context.wrap_socket(server.socket, server_side=True)
        endpoint = f'https://127.0.0.1:{server.server_port}'
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            subprocess.run(['cargo','test','--locked','--manifest-path','src-tauri/Cargo.toml','--features','updater-smoke','--test','updater_macos','--','--ignored','--nocapture'],
                           cwd=repo, env={**os.environ,'STARFOLIO_UPDATER_TEST_FIXTURE':directory,'STARFOLIO_UPDATER_TEST_ENDPOINT':endpoint},check=True)
        finally:
            server.shutdown()
            thread.join()
