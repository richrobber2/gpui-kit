#!/usr/bin/env python3
"""Deploy the Xbox lab through Device Portal's loose-folder workflow."""
import argparse
import base64
from collections import defaultdict
import getpass
import hashlib
import http.cookiejar
import io
import json
from pathlib import Path, PurePosixPath
import ssl
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
import xml.etree.ElementTree as ET
import zipfile

IDENTITY = 'Richard.XboxGpuLab'
NS = {'p': 'http://schemas.microsoft.com/appx/manifest/foundation/windows10'}


def payload(artifact):
    with zipfile.ZipFile(artifact) as archive:
        names = [n for n in archive.namelist()
                 if n.endswith('_x64.appx') and '/Dependencies/' not in n]
        if len(names) != 1:
            raise ValueError('Expected exactly one main x64 APPX in the artifact')
        data = archive.read(names[0])
        dependencies = [(PurePosixPath(n).name, archive.read(n))
                        for n in archive.namelist()
                        if '/Dependencies/x64/' in n and n.endswith('.appx')]
    groups = defaultdict(list)
    with zipfile.ZipFile(io.BytesIO(data)) as package:
        manifest = ET.fromstring(package.read('AppxManifest.xml'))
        identity = manifest.find('p:Identity', NS)
        if (identity.get('Name') != IDENTITY
                or identity.get('ProcessorArchitecture') != 'x64'
                or identity.get('Publisher') != 'CN=XboxGpuLab'):
            raise ValueError('This deployer only supports the x64 Xbox GPU lab')
        for name in package.namelist():
            path = PurePosixPath(name)
            if path.is_absolute() or '..' in path.parts or '\\' in name:
                raise ValueError('Unsafe package path')
            if name.endswith('/') or name in ('AppxBlockMap.xml', 'AppxSignature.p7x', '[Content_Types].xml') or name.startswith('AppxMetadata/'):
                continue
            groups[str(path.parent)].append((path.name, package.read(name)))
    groups['.'].extend(dependencies)
    return groups, hashlib.sha256(data).hexdigest()


class Portal:
    def __init__(self, base, insecure, username):
        parsed = urllib.parse.urlsplit(base)
        if parsed.scheme != 'https' or not parsed.hostname or parsed.username or parsed.query or parsed.fragment:
            raise ValueError('Use the console HTTPS URL without credentials or query parameters')
        self.base = base.rstrip('/')
        self.cookies = http.cookiejar.CookieJar()
        context = ssl._create_unverified_context() if insecure else ssl.create_default_context()
        handlers = [urllib.request.HTTPSHandler(context=context), urllib.request.HTTPCookieProcessor(self.cookies)]
        if username:
            passwords = urllib.request.HTTPPasswordMgrWithDefaultRealm()
            passwords.add_password(None, self.base, username, getpass.getpass('Device Portal password: '))
            handlers.append(urllib.request.HTTPBasicAuthHandler(passwords))
        self.opener = urllib.request.build_opener(*handlers)
        self.request('/')
        info = json.loads(self.request('/api/os/info'))
        if 'xbox' not in info.get('Platform', '').lower():
            raise ValueError('The supplied portal does not identify itself as Xbox')

    def request(self, route, data=None, method='GET', content_type=None):
        headers = {}
        if content_type:
            headers['Content-Type'] = content_type
        if method != 'GET':
            for cookie in self.cookies:
                if cookie.name == 'CSRF-Token':
                    headers['X-CSRF-Token'] = cookie.value
        for attempt in range(3):
            try:
                with self.opener.open(urllib.request.Request(self.base + route, data, headers, method=method), timeout=90) as response:
                    return response.read()
            except urllib.error.HTTPError as error:
                raise RuntimeError(f'{method} {route.split("?")[0]}: HTTP {error.code}: {error.read().decode(errors="replace")}') from error
            except (urllib.error.URLError, TimeoutError):
                if method != 'GET' or attempt == 2:
                    raise
                time.sleep(2)

    def upload(self, folder, files):
        boundary = 'gpui-' + uuid.uuid4().hex
        parts = []
        for name, data in files:
            parts.append((f'--{boundary}\r\nContent-Disposition: form-data; name="{name}"; filename="{name}"\r\nContent-Type: application/octet-stream\r\n\r\n').encode() + data + b'\r\n')
        parts.append(f'--{boundary}--\r\n'.encode())
        self.request('/api/app/packagemanager/upload?' + urllib.parse.urlencode({'destinationFolder': encoded(folder)}), b''.join(parts), 'POST', 'multipart/form-data; boundary=' + boundary)


def encoded(value):
    return base64.b64encode(value.encode()).decode()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('artifact', type=Path)
    parser.add_argument('--portal', required=True)
    parser.add_argument('--username')
    parser.add_argument('--insecure', action='store_true', help='Accept the local console self-signed TLS certificate')
    parser.add_argument('--dry-run', action='store_true', help='Validate and list the artifact without contacting the console')
    args = parser.parse_args()
    groups, digest = payload(args.artifact)
    folder = 'GpuiKitXbox-' + digest[:12]
    if args.dry_run:
        print(f'{folder}: {sum(len(files) for files in groups.values())} files; APPX SHA256 {digest}')
        return
    portal = Portal(args.portal, args.insecure, args.username)
    packages = json.loads(portal.request('/api/app/packagemanager/packages'))['InstalledPackages']
    for package in packages:
        name = package.get('PackageFullName', '')
        if name.startswith(IDENTITY + '_'):
            portal.request('/api/taskmanager/app?' + urllib.parse.urlencode({'package': encoded(name)}), method='DELETE')
    for parent, files in groups.items():
        destination = folder if parent == '.' else folder + '/' + parent
        print('Uploading', destination, flush=True)
        portal.upload(destination, files)
    portal.request('/api/app/packagemanager/register?' + urllib.parse.urlencode({'folder': encoded(folder)}), b'', 'POST')
    deadline = time.monotonic() + 90
    while True:
        body = portal.request('/api/app/packagemanager/state')
        if body:
            state = json.loads(body)
            if not state.get('Success'):
                raise RuntimeError(str(state))
            break
        if time.monotonic() > deadline:
            raise TimeoutError('Package registration is still pending')
        time.sleep(1)
    packages = json.loads(portal.request('/api/app/packagemanager/packages'))['InstalledPackages']
    package = next(p for p in packages if p.get('PackageFullName', '').startswith(IDENTITY + '_'))
    portal.request('/api/taskmanager/app?' + urllib.parse.urlencode({'appid': encoded(package['PackageRelativeId'])}), b'', 'POST')
    print('Launched', package['PackageRelativeId'])
    print('Developer files:', folder)
    print('Check the rendered screen and LocalState/native-gpu-result.json for hardware verification.')


if __name__ == '__main__':
    main()
