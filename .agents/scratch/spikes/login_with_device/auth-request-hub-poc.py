#!/data/data/com.termux/files/usr/bin/python3
"""Proof: wait for the approval on Vaultwarden's anonymous hub instead of polling.

Sends an auth request, opens wss://<base>/notifications/anonymous-hub?Token=<id>
(SignalR MessagePack handshake, answers server pings), waits for
"AuthRequestResponseRecieved", then makes a single GET .../response and checks
the returned user key against rbw's local db. Polling is avoided because
Vaultwarden >= 1.37.3 rate-limits that GET (burst 50, then 1/min per IP).

Run from anywhere: auth-request-hub-poc.py <timeout_seconds>. Read-only for rbw.
"""
import base64, http.client, importlib.util, json, os, secrets, socket, ssl, string, struct, subprocess, sys, tempfile, time, urllib.parse

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("poc", os.path.join(HERE, "auth-request-poc.py"))
poc = importlib.util.module_from_spec(spec); spec.loader.exec_module(poc)  # forces IPv4 too

timeout = int(sys.argv[1])
cfg = json.loads(subprocess.run(["rbw", "config", "show"], check=True, capture_output=True, text=True).stdout)
email, base_url = cfg["email"], cfg["base_url"].rstrip("/")
data_home = os.environ.get("XDG_DATA_HOME", os.path.expanduser("~/.local/share"))
cache_home = os.environ.get("XDG_CACHE_HOME", os.path.expanduser("~/.cache"))
device_id = open(os.path.join(data_home, "rbw", "device_id")).read().strip()
db = json.load(open(os.path.join(cache_home, "rbw", urllib.parse.quote(base_url, safe="") + ":" + email + ".json")))
host = urllib.parse.urlparse(base_url).hostname
hdr = {"Content-Type": "application/json", "Device-Type": "8", "Bitwarden-Client-Name": "cli",
       "Bitwarden-Client-Version": "1.15.0", "User-Agent": "rbw-hub-probe"}


def call(method, path, body=None):
    conn = http.client.HTTPSConnection(host, timeout=30)
    conn.request(method, path, json.dumps(body) if body else None, hdr)
    r = conn.getresponse(); p = r.read().decode(); conn.close()
    return r.status, p


def ws_open(path):
    raw = socket.create_connection((host, 443), timeout=30)
    s = ssl.create_default_context().wrap_socket(raw, server_hostname=host)
    key = base64.b64encode(os.urandom(16)).decode()
    s.sendall((f"GET {path} HTTP/1.1\r\nHost: {host}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
               f"Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n").encode())
    head = b""
    while b"\r\n\r\n" not in head:
        head += s.recv(1)
    return s, head.split(b"\r\n", 1)[0].decode()


def ws_send(s, opcode, payload):
    mask = os.urandom(4)
    n = len(payload)
    hdr_ = bytes([0x80 | opcode]) + (bytes([0x80 | n]) if n < 126 else bytes([0x80 | 126]) + struct.pack(">H", n))
    s.sendall(hdr_ + mask + bytes(b ^ mask[i % 4] for i, b in enumerate(payload)))


def recv_exact(s, n):
    b = b""
    while len(b) < n:
        chunk = s.recv(n - len(b))
        if not chunk:
            raise EOFError("closed")
        b += chunk
    return b


def ws_recv(s):
    b0, b1 = recv_exact(s, 2)
    n = b1 & 0x7F
    if n == 126: n = struct.unpack(">H", recv_exact(s, 2))[0]
    if n == 127: n = struct.unpack(">Q", recv_exact(s, 8))[0]
    return b0 & 0x0F, recv_exact(s, n)


with tempfile.TemporaryDirectory() as w:
    priv = os.path.join(w, "p.pem")
    subprocess.run(["openssl", "genpkey", "-algorithm", "RSA", "-pkeyopt", "rsa_keygen_bits:2048", "-out", priv], check=True, capture_output=True)
    spki = subprocess.run(["openssl", "pkey", "-in", priv, "-pubout", "-outform", "DER"], check=True, capture_output=True).stdout
    code = "".join(secrets.choice(string.ascii_letters + string.digits) for _ in range(25))
    st, p = call("POST", "/api/auth-requests", {"email": email, "publicKey": base64.b64encode(spki).decode(),
                 "deviceIdentifier": device_id, "accessCode": code, "type": 0})
    req = json.loads(p)
    t0 = time.time()
    print(f"[hub] POST {st} phrase: {poc.fingerprint_phrase(email, spki)}", flush=True)
    s, status = ws_open(f"/notifications/anonymous-hub?Token={req['id']}")
    print(f"[hub] upgrade: {status}", flush=True)
    ws_send(s, 0x1, b'{"protocol":"messagepack","version":1}\x1e')
    s.settimeout(timeout)
    frames = 0
    while time.time() - t0 < timeout:
        op, data = ws_recv(s)
        frames += 1
        if op == 0x9:
            ws_send(s, 0xA, data); continue
        print(f"[hub] t={time.time()-t0:6.1f}s frame op={op} len={len(data)} head={data[:60]!r}", flush=True)
        if op == 0x8 or b"AuthRequestResponseRecieved" in data:
            break
    st, p = call("GET", f"/api/auth-requests/{req['id']}/response?code={code}")
    print(f"[hub] GET {st} after {time.time()-t0:.1f}s, frames={frames}", flush=True)
    res = json.loads(p)
    kind, blob = res["key"].split(".", 1)
    md = {"3": "sha256", "4": "sha1"}[kind]
    key = subprocess.run(["openssl", "pkeyutl", "-decrypt", "-inkey", priv, "-pkeyopt", "rsa_padding_mode:oaep",
                          "-pkeyopt", f"rsa_oaep_md:{md}", "-pkeyopt", f"rsa_mgf1_md:{md}"],
                         input=base64.b64decode(blob), check=True, capture_output=True).stdout
    print(f"[hub] key type {kind}, {len(key)} bytes, user key HMAC OK: {poc.mac_matches(db['protected_private_key'], key[32:])}")
