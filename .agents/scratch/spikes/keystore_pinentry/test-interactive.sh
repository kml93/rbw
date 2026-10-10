#!/data/data/com.termux/files/usr/bin/bash
# Throwaway end-to-end test with the real rbw, fully isolated in a temp dir:
# own config, own agent, own keystore key. The real rbw setup is untouched.
set -u
P=$(cd "$(dirname "$0")" && pwd)
email=$(rbw config show | jq -r .email)
base_url=$(rbw config show | jq -r .base_url)

W=$(mktemp -d)
export XDG_CONFIG_HOME="$W/config" XDG_CACHE_HOME="$W/cache" XDG_DATA_HOME="$W/data" XDG_RUNTIME_DIR="$W/run"
export RBW_PROFILE=biotest BIO_ALIAS=rbw-biotest BIO_DIR="$W/bio"
mkdir -p "$XDG_RUNTIME_DIR" && chmod 700 "$XDG_RUNTIME_DIR"
cleanup() {
  rbw stop-agent 2>/dev/null
  termux-keystore delete "$BIO_ALIAS" 2>/dev/null
  rm -rf "$W"
  echo "== nettoyé"
}
trap cleanup EXIT

rbw config set email "$email"
rbw config set base_url "$base_url"
rbw config set pinentry "$P/pinentry-bio"

echo "== 1/3 scellement : tape ton master password, puis empreinte"
"$P/rbw-bio-seal" || exit 1

echo "== 2/3 rbw login : empreinte (et code 2FA si ton compte en a un)"
rbw login || exit 1
rbw unlocked && echo "-> déverrouillé, $(rbw list | wc -l) entrées"

echo "== 3/3 rbw lock puis rbw unlock : empreinte seule, aucun mot de passe"
rbw lock
rbw unlock || exit 1
rbw unlocked && echo "-> déverrouillé à l'empreinte, $(rbw list | wc -l) entrées"
