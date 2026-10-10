# Déverrouillage biométrique de rbw

État au 2026-10-10. Rien n'est implémenté dans rbw : le travail est à l'étape compréhension et preuves.

## Objectif

Déverrouiller `rbw-agent` sans taper le master password et sans le stocker côté client. La cible principale est Termux sur Android, mais le mécanisme retenu ne dépend pas de la plateforme. Le serveur est un Vaultwarden auto-hébergé ; son `base_url` est dans `rbw config show`.

## Décision

**Option B retenue.** Les *auth requests* de Vaultwarden (« Log in with device ») servent à **déverrouiller** rbw, pas à s'y connecter. L'appli Bitwarden mobile reçoit un push, l'utilisateur approuve, et rbw reçoit la user key chiffrée pour sa clé RSA éphémère.

Les alternatives sont gardées comme prototypes, sans plan pour les mettre en place :
- **A** (`spikes/keystore_pinentry`) : pinentry à l'empreinte avec le keystore Android de Termux. Le master password est stocké chiffré.
- **C** : APK Android dédiée avec `BiometricPrompt` + `CryptoObject`. Elle n'est pas écrite.

Pourquoi B : c'est la seule option où le master password n'existe nulle part sur le client.

## Preuves sur appareil réel

**B** (`spikes/login_with_device/auth-request-poc.py`) :
- La demande a été approuvée dans l'appli Bitwarden en 17,5 s.
- Le champ `key` reçu est un `EncString` type 4 (RSA-OAEP-SHA1). Une fois déchiffré, il fait 64 octets : c'est la user key.
- Elle est vérifiée par HMAC contre `protected_private_key` dans la base locale de rbw : OK.
- `masterPasswordHash` vaut `null`. L'appli envoie donc la user key, pas la master key.
- La phrase d'empreinte reproduit le vecteur de test du SDK Bitwarden.
- L'utilisateur a confirmé que la phrase affichée dans l'appli Bitwarden était identique à celle du script.

**A** (`spikes/keystore_pinentry/`) :
- `termux-fingerprint` fonctionne sur Android 15.
- Une clé `termux-keystore generate … -u 10` refuse de signer sans authentification. Elle signe juste après l'empreinte et refuse de nouveau 12 s plus tard.
- `SHA256withRSA` est déterministe.
- Les tests T1 à T5 passent (`test-noninteractive.sh`).

## Faits techniques et sources

**Vaultwarden** (`dani-garcia/vaultwarden` @ `df2cd3c`) :
- `src/api/core/accounts.rs`, `post_auth_request` (~l.1663) : appel sans authentification. L'appareil `deviceIdentifier` doit déjà exister pour l'utilisateur avec le même en-tête `Device-Type`. Le corps est en camelCase : `email`, `publicKey`, `deviceIdentifier`, `accessCode`. Le champ `type` est ignoré.
- `get_auth_request_response` (~l.1827) : appel sans authentification. Vaultwarden vérifie le type d'appareil, l'IP de la demande et l'access code. La réponse contient `key`, `masterPasswordHash` et `requestApproved`.
- `src/api/identity.rs` ~l.481 : un login par auth request appelle quand même `twofactor_auth`. B ne remplace donc pas la 2FA au login, il sert seulement au déverrouillage.

**rbw** (`doy/rbw` 1.15.0, `e673049`) :
- `src/api.rs` ~l.814-817 : `BITWARDEN_CLIENT="cli"`, `DEVICE_TYPE=8`.
- L'identifiant d'appareil est dans `$XDG_DATA_HOME/rbw/device_id` (`src/dirs.rs` l.62, `src/config.rs` l.219).
- Point d'insertion dans `src/bin/rbw-agent/actions.rs` : `unlock_state` (l.374-459) passe par le pinentry, puis appelle `rbw::actions::unlock` (`src/actions.rs` l.78), puis `unlock_success` (l.473). Il faut un chemin qui part directement de la user key.
- `src/pinentry.rs` : rbw envoie toutes les commandes, ferme stdin, puis lit la réponse. La ligne `D` est décodée en percent-encoding.
- Le crate `rsa` est déjà une dépendance (`Cargo.toml` l.57).
- La base locale est `$XDG_CACHE_HOME/rbw/<base_url urlencodé>:<email>.json`. `protected_key` est un type 2 (master key étirée) et `protected_private_key` un type 2 (user key). La KDF est Argon2id.

**termux-api** :
- `KeystoreAPI` ne propose que `list`, `generate`, `delete`, `sign` et `verify`.
- `-u` correspond à `setUserAuthenticationValidityDurationSeconds`. La fenêtre s'ouvre aussi quand l'utilisateur déverrouille le téléphone.
- `FingerprintAPI` utilise `BiometricPrompt` sans `CryptoObject` : la réponse n'est qu'un booléen.
- La PR termux-api #556 (encrypt/decrypt) n'est pas mergée.

**Autres** :
- Le fork `pschmitt/rbw` fait un déverrouillage keystore natif avec le mécanisme de A (`src/termux.rs`). Il ne gère pas les auth requests.
- Le CLI `bw` ne gère pas « login with device ».
- La phrase d'empreinte est définie dans `bitwarden/sdk-internal`, `crates/bitwarden-crypto/src/fingerprint.rs` : HKDF-expand avec prk = clé publique SPKI et info = email, puis 5 mots EFF.

**Paquets** : Arch (`extra/rbw`, PKGBUILD `git+https://github.com/doy/rbw.git#tag=$pkgver`) et Termux (`packages/rbw/build.sh`, tarball du tag `doy/rbw`, aucun patch) compilent tous les deux le tag upstream 1.15.0. Les binaires diffèrent (glibc x86_64 contre bionic aarch64), mais la source est la même.

## Environnement de développement

- B ne contient rien de spécifique à Termux. On peut le développer et le tester sur PC (Linux), contre le vrai Vaultwarden, en approuvant depuis le téléphone.
- Termux sert à l'intégration finale et à A (`termux-api`).
- Il existe un conteneur `termux/termux-docker` (`aarch64`, `arm`, `i686`, `x86_64`), mais sans composants Android, donc sans `termux-api`. Sur un PC x86, la variante ARM passe par QEMU (lent).
- Pour un binaire Termux : compiler sur le téléphone (rustc 1.97.1 installé) ou cibler `aarch64-linux-android` avec le NDK Android.
- Sur Termux, `XDG_RUNTIME_DIR` n'est pas défini : rbw-agent met donc son socket dans `$TMPDIR/rbw-<uid>`. `$TMPDIR` a déjà été vidé une fois pendant que l'agent tournait, et l'agent est devenu injoignable.

## À vérifier avant d'implémenter B

1. Compiler `main` tel quel (`cargo build --release`) sur la machine cible.
2. Chercher le travail existant : issues et PR upstream sur « login with device » ou « auth request », et les forks `koterpillar/rbw`, `devgar/rbw`, `bearcove/rbw`. Option : ouvrir une issue upstream pour valider le design avant de coder.
3. Chemins limites avec le PoC : refus (`requestApproved=false`), délai dépassé, appli Bitwarden verrouillée ou fermée, mode hors ligne (il faut retomber sur le master password).
4. Vérifier quel `AuthRequestType` les clients officiels utilisent pour un simple déverrouillage : 0 (AuthenticateAndUnlock) ou 1 (Unlock). Vaultwarden ignore ce champ, mais pas forcément le cloud Bitwarden.
5. Durée de vie du refresh token sur Vaultwarden, pour savoir tous les combien `rbw login` et la 2FA reviennent.
6. Question de design : comment montrer la phrase d'empreinte, alors que rbw-agent n'a pas de tty. Pistes : le pinentry (`SETDESC` / `MESSAGE`), une notification, ou un affichage côté client `rbw`.

## Plan proposé pour B (esquisse, environ 300-400 lignes de Rust, estimation)

1. Ajouter une option de config pour choisir le déverrouillage par appareil (nom à définir).
2. Dans `unlock_state`, si l'option est active :
   1. générer une paire RSA-2048 et un `accessCode` de 25 caractères alphanumériques ;
   2. envoyer `POST /api/auth-requests` avec le `device_id` et `Device-Type: 8` de rbw ;
   3. montrer la phrase d'empreinte à l'utilisateur ;
   4. interroger `GET …/response?code=` toutes les 3 s, pendant 5 min maximum ;
   5. déchiffrer `key` (type 4 : OAEP-SHA1, type 3 : OAEP-SHA256) ;
   6. construire les `Keys` à partir des 64 octets ;
   7. reprendre le code existant pour `protected_private_key` et les clés d'organisation.
3. Sur un refus, une erreur ou en mode hors ligne, revenir au master password via pinentry.

## Hors du fork (à faire ailleurs, plus tard)

- Config rbw de l'utilisateur : un `lock_timeout` plus court une fois B en place.
- Installation du binaire patché sans qu'un `pkg upgrade` l'écrase.
- Appli Bitwarden : délai de verrouillage « Immédiatement » si l'utilisateur veut poser le doigt à chaque approbation.
- Notes sur l'infrastructure de l'utilisateur : `.agents/scratch/local/` (non suivi par git).

## Skills suggérés

- `coding-rules` : avant de modifier le code de rbw.
- `codebase-design` : pour concevoir le point d'insertion du déverrouillage par appareil dans `rbw-agent`.
- `tdd` : tests Rust (déchiffrement, phrase d'empreinte, déverrouillage par user key).
- `research` : issues et forks upstream, `AuthRequestType` des clients officiels.
