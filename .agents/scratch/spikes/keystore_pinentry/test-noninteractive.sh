#!/data/data/com.termux/files/usr/bin/bash
# Throwaway checks that need no finger: fake termux-fingerprint, fake fallback.
set -u
P=$(cd "$(dirname "$0")" && pwd)
W=$(mktemp -d)
mkdir -p "$W/bin"
printf '#!/bin/sh\necho %s\n' "'{\"auth_result\": \"AUTH_RESULT_SUCCESS\"}'" >"$W/bin/termux-fingerprint"
printf '#!/bin/sh\ncat >/dev/null\necho "OK fake"; echo OK; echo OK; echo OK; echo "D FALLBACK"; echo OK\n' >"$W/bin/fake-pinentry"
chmod +x "$W/bin/"*
export PATH="$W/bin:$PATH" PINENTRY_FALLBACK=fake-pinentry BIO_DIR="$W/vault"

RBW_MASTER=$'SETTITLE rbw\nSETPROMPT Master Password\nSETDESC Unlock the local database for \'x\'\nGETPIN'
RBW_RETRY=$'SETTITLE rbw\nSETPROMPT Master Password\nSETDESC d\nSETERROR Invalid master password\nGETPIN'
RBW_2FA=$'SETTITLE rbw\nSETPROMPT Authenticator App\nSETDESC Enter the 6 digit verification code\nGETPIN'

echo "== T1 key without auth window: seal then unseal through the Assuan dialogue"
termux-keystore generate biotest_noauth -a RSA -s 2048
termux-keystore generate biotest_u -a RSA -s 2048 -u 10
printf '%s\n' 'p@ss%w0rd é' | BIO_ALIAS=biotest_noauth "$P/rbw-bio-seal"; echo "seal rc=$?"
ls -l "$W/vault"
printf '%s\n' "$RBW_MASTER" | BIO_ALIAS=biotest_noauth "$P/pinentry-bio" --timeout 0
echo "== T2 retry after wrong password -> fallback"
printf '%s\n' "$RBW_RETRY" | BIO_ALIAS=biotest_noauth "$P/pinentry-bio" | grep '^D'
echo "== T3 2FA prompt -> fallback"
printf '%s\n' "$RBW_2FA" | BIO_ALIAS=biotest_noauth "$P/pinentry-bio" | grep '^D'
echo "== T4 auth-bound key, fake fingerprint, no real auth -> sign refused -> fallback"
BIO_ALIAS=biotest_u BIO_DIR="$W/vault" "$P/pinentry-bio" <<<"$RBW_MASTER" | grep '^D'
echo "== T5 seal with auth-bound key and fake fingerprint -> must refuse"
printf '%s\n' 'x' | BIO_ALIAS=biotest_u BIO_DIR="$W/vault2" "$P/rbw-bio-seal"; echo "seal rc=$?"
termux-keystore delete biotest_noauth
termux-keystore delete biotest_u
rm -rf "$W"
