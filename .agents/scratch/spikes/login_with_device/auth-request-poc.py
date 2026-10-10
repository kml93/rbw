#!/data/data/com.termux/files/usr/bin/python3
"""Throwaway proof: unlock-by-approval on Vaultwarden, without rbw changes.

Sends a "log in with device" request with rbw's registered device id, waits for
the approval in the Bitwarden app, decrypts the returned key with a throwaway
RSA key, and proves it is the right vault key by checking the HMAC of a value
from rbw's local db. Prints no secret. Read-only for rbw.
"""

import base64
import hashlib
import hmac
import http.client
import json
import os
import secrets
import socket
import string
import subprocess
import sys
import tempfile
import time
import urllib.parse

HERE = os.path.dirname(os.path.abspath(__file__))
WORDLIST = os.path.join(HERE, "eff_large_wordlist.txt")
DEVICE_TYPE = "8"  # rbw registers itself as LinuxDesktop (rbw src/api.rs)
POLL_SECONDS, TIMEOUT_SECONDS = 3, 240

# Vaultwarden ties the request to the client IP: stay on one IPv4 connection.
_getaddrinfo = socket.getaddrinfo
socket.getaddrinfo = lambda *a, **k: [r for r in _getaddrinfo(*a, **k) if r[0] == socket.AF_INET]


def hkdf_expand(prk: bytes, info: bytes, length: int) -> bytes:
    out, block, counter = b"", b"", 1
    while len(out) < length:
        block = hmac.new(prk, block + info + bytes([counter]), hashlib.sha256).digest()
        out += block
        counter += 1
    return out[:length]


def fingerprint_phrase(material: str, spki: bytes) -> str:
    words = [line.split("\t")[1].strip() for line in open(WORDLIST)]
    number = int.from_bytes(hkdf_expand(spki, material.encode(), 32), "big")
    phrase = []
    for _ in range(5):
        number, index = divmod(number, len(words))
        phrase.append(words[index])
    return "-".join(phrase)


def mac_matches(enc_string: str, mac_key: bytes) -> bool:
    kind, rest = enc_string.split(".", 1)
    assert kind == "2", f"unexpected EncString type {kind}"
    iv, data, mac = (base64.b64decode(p) for p in rest.split("|"))
    return hmac.compare_digest(hmac.new(mac_key, iv + data, hashlib.sha256).digest(), mac)


def self_test() -> None:
    # Test vector from bitwarden/sdk-internal crates/bitwarden-crypto/src/fingerprint.rs
    spki = bytes([
        48, 130, 1, 34, 48, 13, 6, 9, 42, 134, 72, 134, 247, 13, 1, 1, 1, 5, 0, 3, 130, 1, 15,
        0, 48, 130, 1, 10, 2, 130, 1, 1, 0, 187, 38, 44, 241, 110, 205, 89, 253, 25, 191, 126,
        84, 121, 202, 61, 223, 189, 244, 118, 212, 74, 139, 130, 97, 115, 164, 167, 106, 191,
        188, 233, 218, 196, 250, 187, 146, 125, 160, 150, 49, 198, 224, 176, 10, 0, 143, 99,
        230, 232, 160, 51, 104, 154, 211, 33, 80, 170, 4, 68, 80, 219, 115, 167, 114, 156, 227,
        125, 193, 128, 123, 39, 254, 191, 124, 63, 129, 44, 63, 18, 56, 161, 48, 158, 0, 27,
        146, 2, 99, 136, 75, 21, 135, 6, 118, 12, 26, 251, 184, 172, 249, 53, 78, 210, 46, 143,
        17, 104, 202, 65, 173, 229, 219, 233, 144, 163, 101, 216, 238, 152, 54, 158, 1, 195,
        50, 203, 21, 226, 12, 82, 170, 175, 170, 160, 21, 247, 248, 80, 97, 123, 0, 152, 116,
        229, 126, 221, 199, 155, 194, 192, 51, 207, 177, 240, 160, 84, 241, 41, 88, 176, 53,
        111, 28, 173, 177, 232, 158, 22, 79, 133, 152, 31, 32, 12, 196, 147, 58, 57, 50, 252,
        208, 131, 150, 179, 132, 178, 150, 234, 251, 143, 125, 163, 144, 20, 46, 71, 168, 252,
        164, 86, 120, 124, 56, 252, 206, 210, 236, 212, 139, 127, 189, 236, 40, 46, 2, 238, 13,
        216, 40, 48, 85, 133, 229, 181, 155, 176, 217, 241, 154, 153, 213, 112, 222, 72, 219,
        197, 3, 219, 56, 77, 109, 47, 72, 251, 131, 36, 240, 96, 169, 31, 82, 93, 166, 242, 3,
        33, 213, 2, 3, 1, 0, 1,
    ])
    got = fingerprint_phrase("a09726a0-9590-49d1-a5f5-afe300b6a515", spki)
    assert got == "turban-deftly-anime-chatroom-unselfish", got


def main() -> int:
    self_test()
    config = json.loads(subprocess.run(["rbw", "config", "show"], check=True, capture_output=True, text=True).stdout)
    email, base_url = config["email"], config["base_url"].rstrip("/")
    data_home = os.environ.get("XDG_DATA_HOME", os.path.expanduser("~/.local/share"))
    cache_home = os.environ.get("XDG_CACHE_HOME", os.path.expanduser("~/.cache"))
    device_id = open(os.path.join(data_home, "rbw", "device_id")).read().strip()
    db_name = urllib.parse.quote(base_url, safe="") + ":" + email + ".json"
    db = json.load(open(os.path.join(cache_home, "rbw", db_name)))
    if "--dry-run" in sys.argv:
        print("dry-run: config, device_id, db and fingerprint self-test OK; nothing sent")
        return 0

    with tempfile.TemporaryDirectory() as work:
        priv = os.path.join(work, "priv.pem")
        subprocess.run(["openssl", "genpkey", "-algorithm", "RSA", "-pkeyopt", "rsa_keygen_bits:2048", "-out", priv],
                       check=True, capture_output=True)
        spki = subprocess.run(["openssl", "pkey", "-in", priv, "-pubout", "-outform", "DER"],
                              check=True, capture_output=True).stdout
        access_code = "".join(secrets.choice(string.ascii_letters + string.digits) for _ in range(25))
        phrase = fingerprint_phrase(email, spki)

        host = urllib.parse.urlparse(base_url).hostname
        conn = http.client.HTTPSConnection(host, timeout=30)
        headers = {"Content-Type": "application/json", "Device-Type": DEVICE_TYPE,
                   "Bitwarden-Client-Name": "cli", "Bitwarden-Client-Version": "1.15.0",
                   "User-Agent": "rbw-auth-request-poc"}

        def call(method: str, path: str, body: dict | None = None) -> dict:
            conn.request(method, path, json.dumps(body) if body else None, headers)
            resp = conn.getresponse()
            payload = resp.read().decode()
            if resp.status != 200:
                raise SystemExit(f"{method} {path.split('?')[0]} -> HTTP {resp.status}: {payload[:300]}")
            return json.loads(payload)

        req = call("POST", "/api/auth-requests", {
            "email": email, "publicKey": base64.b64encode(spki).decode(), "deviceIdentifier": device_id,
            "accessCode": access_code, "type": 0, "fingerprintPhrase": phrase})
        print(f"Demande envoyée (IP vue par le serveur : {req['requestIpAddress']}).")
        print(f"Phrase d'empreinte attendue dans l'appli Bitwarden : {phrase}")
        print(f"J'attends ton approbation (max {TIMEOUT_SECONDS} s)...", flush=True)

        deadline = time.time() + TIMEOUT_SECONDS
        while time.time() < deadline:
            res = call("GET", f"/api/auth-requests/{req['id']}/response?code={access_code}")
            if res["requestApproved"] is False:
                print("Demande refusée dans l'appli.")
                return 1
            if res["requestApproved"] and res["key"]:
                break
            time.sleep(POLL_SECONDS)
        else:
            print("Délai dépassé, aucune réponse.")
            return 1

        kind, blob = res["key"].split(".", 1)
        md = {"3": "sha256", "4": "sha1"}[kind]  # Rsa2048_OaepSha256_B64 / Rsa2048_OaepSha1_B64
        key = subprocess.run(["openssl", "pkeyutl", "-decrypt", "-inkey", priv,
                              "-pkeyopt", "rsa_padding_mode:oaep", "-pkeyopt", f"rsa_oaep_md:{md}",
                              "-pkeyopt", f"rsa_mgf1_md:{md}"],
                             input=base64.b64decode(blob), check=True, capture_output=True).stdout
        print(f"Approuvé. Clé reçue : EncString type {kind}, {len(key)} octets déchiffrés,"
              f" masterPasswordHash fourni : {res['masterPasswordHash'] is not None}")
        if len(key) == 64:
            ok = mac_matches(db["protected_private_key"], key[32:])
            print(f"User key (64 o) vérifiée contre protected_private_key de rbw : {'OK' if ok else 'ÉCHEC'}")
        else:
            ok = mac_matches(db["protected_key"], hkdf_expand(key, b"mac", 32))
            print(f"Master key (32 o) vérifiée contre protected_key de rbw : {'OK' if ok else 'ÉCHEC'}")
        return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
