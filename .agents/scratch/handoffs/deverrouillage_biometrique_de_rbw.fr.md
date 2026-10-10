# Déverrouillage biométrique de rbw

État au 2026-10-10. B est implémenté sur `kml93` (option `unlock_with_device`), avec la PR doy/rbw#378 (refresh token) mergée dans la branche. Revue faite, correctif IP fait. Binaires installés dans `$XDG_BIN_HOME` (avant `/usr/bin` dans le `PATH`, donc à l'abri de `pkg upgrade`) ; option activée dans la vraie config. Reste : usage quotidien, puis PR upstream (branche depuis `main`, sans #378 ni `.agents/`).

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

## Résultats des vérifications (2026-10-10)

1. **Build** : `main` ne compile pas avec les features par défaut sur Termux (`arboard`, le presse-papiers, échoue sur Android). Le paquet Termux compile avec `--no-default-features` ; c'est ce qu'il faut utiliser. L'environnement de l'utilisateur exporte `RUSTFLAGS=-C target-cpu=native`, qui produit un binaire tué par SIGILL (instruction illégale) sur ce SoC big.LITTLE. Commande fiable : `RUSTFLAGS= CARGO_TARGET_DIR=target cargo build --release --no-default-features` (≈ 3,5 min).
2. **Travail existant** : aucune issue, PR ou fork (`koterpillar`, `devgar`, `bearcove`, `pschmitt`, `andrewgazelka`) ne gère les auth requests. Issue liée ouverte : doy/rbw#324 « Handle biometrics ». Position de doy sur les fonctions d'auth : « happy to review prs » (#118, SSO, mergé via #174).
3. **Chemins limites** (scripts jetables, serveur réel) :
   - Refus : Vaultwarden **supprime** la demande et n'envoie aucune notification. Le GET suivant renvoie `400 "AuthRequest doesn't exist"`. `requestApproved=false` n'apparaît jamais.
   - Expiration : purge à 15 min (observée à 952 s), même `400` qu'un refus. Les deux sont indiscernables.
   - Appli tuée (arrêt forcé Android) : la notification push arrive en retard, mais la demande s'affiche à l'ouverture de l'appli. Approuvée en 108 s.
   - **Limite de débit** : Vaultwarden ≥ 1.37.3 applique `check_limit_unauthenticated` (rafale de 50, puis 1 requête/min par IP, partagée avec `prelogin`, etc.) à `POST /api/auth-requests` **et** à `GET …/response`. Interroger toutes les 3 s donne `429` au bout de ~60 s. **Interroger en boucle est exclu.**
   - Hors ligne : une erreur réseau sur le POST ou le WebSocket veut dire pas de chemin appareil ; on garde seulement le pinentry (à vérifier en smoke test Rust).
   - Doublon côté appli : après chaque approbation, l'appli Bitwarden affiche une seconde demande avec la même phrase (probablement le push type 16 `AuthRequestResponse` envoyé à tous les appareils). Y répondre donne une erreur, sans effet serveur. Rien à faire côté rbw.
4. **AuthRequestType** : les clients officiels n'envoient que 0 (AuthenticateAndUnlock) ; 1 (Unlock) est défini mais jamais produit. Le cloud refuse un type 1 comme identifiant de login, sans autre différence. → envoyer **0**. Sur le cloud, `requestApproved` vaut `false` aussi bien en attente que refusé : se fier à `responseDate` et `key`. Le cloud exige un en-tête `Device-Type` et, par défaut, un `deviceIdentifier` connu.
5. **Refresh token** : JWT de 30 jours (90 pour les appareils mobiles ; rbw se déclare type 8, donc 30). Vaultwarden en renvoie un nouveau à chaque refresh, mais `ConnectRefreshTokenRes` (`src/api.rs` l.373) ne garde que `access_token` : rbw **jette** le token renouvelé. Résultat : `rbw login` + 2FA tous les 30 jours après le login, même avec un usage quotidien. C'est un oubli (commit `201d2d4` de 2020, sans justification), pas un choix : tous les clients officiels (web, extension, desktop, `bw`, Android) enregistrent le token renvoyé. **Le correctif existe déjà en PR upstream : doy/rbw#378** (fredrikekre, ouverte le 2026-09-14, sans revue de doy). Liées : #342, #362, #32. Ne pas ouvrir de doublon. La case 2FA « se souvenir de moi » est un autre mécanisme : un jeton fixe de 30 jours qui évite la 2FA au prochain login complet ; rbw n'envoie pas `twoFactorRemember`.
6. **Attente de l'approbation sans interroger** : Vaultwarden notifie l'approbation sur le hub **anonyme** `wss://<base>/notifications/anonymous-hub?Token=<id de la demande>` (sans authentification ni limite de débit, 25 connexions max par IP). Poignée de main SignalR MessagePack identique au hub authentifié de rbw (`src/bin/rbw-agent/notifications.rs`) ; message `[1, {}, nil, "AuthRequestResponseRecieved", [{Type: 16, Payload: {Id, UserId}, …}]]` (faute « Recieved » d'origine). Le serveur envoie un ping WebSocket toutes les 15 s ; `main` ferme les clients muets depuis 30 s. Rien n'est envoyé au refus. Prouvé par `spikes/login_with_device/auth-request-hub-poc.py` : approbation reçue en 28,6 s, un seul GET, user key vérifiée.
7. **Affichage de la phrase** : `pinentry-curses` tué par SIGTERM en pleine saisie restaure le terminal. Une seule fenêtre pinentry peut donc montrer la phrase et accepter le master password, puis être fermée par l'agent à l'approbation.

## Implémentation de B (branche `kml93`)

- Option `unlock_with_device` (booléen, `false` par défaut) ; documentée dans `README.md` et `CHANGELOG.md`.
- `src/fingerprint.rs` : phrase d'empreinte. Réutilise la liste EFF déjà présente (`src/wordlist.rs`, identique mot pour mot à celle du spike). Testée avec le vecteur du SDK.
- `src/api.rs` : `create_auth_request` (type 0) et `auth_request_key` (un seul GET, endpoint limité en débit).
- `src/actions.rs` : `unlock_with_user_key`, extrait de `unlock` (comportement du master password inchangé).
- `src/cipherstring.rs` : `decrypt_locked_rsa`, extrait de `decrypt_locked_asymmetric`, pour la clé éphémère.
- `src/bin/rbw-agent/notifications.rs` : seul propriétaire du format SignalR (connexion + poignée de main, ping, lecture de `AuthRequestResponseRecieved`).
- `src/bin/rbw-agent/device_approval.rs` : `PendingApproval::request` (RSA-2048, code d'accès, POST, hub anonyme), `fingerprint_phrase`, `approved_user_key` (attente avec ping SignalR toutes les 15 s, délai de 15 min, GET, déchiffrement, contrôle des 64 octets).
- `unlock_state` : `tokio::select!` entre le pinentry et l'approbation. La clé approuvée est validée **dans** la future d'approbation : en cas d'échec, cette future ne se termine jamais et le pinentry reste ouvert. La demande est créée avec un délai max de 10 s, pour ne pas retarder le pinentry.
- `src/pinentry.rs` : `TerminateOnDrop` envoie SIGTERM au pinentry quand l'approbation gagne. Les textes `SETDESC` et `SETERROR` sont maintenant encodés en Assuan (`%`, `\r`, `\n`).
- Revue par 2 agents (correction + coding-rules) appliquée. Point écarté : la gestion des statuts HTTP reste en ligne (convention rbw).
- Correctif IP : `create_auth_request` renvoie un `rbw::api::AuthRequest` (id + adresse serveur du POST via `remote_addr()`) ; `auth_request_key` force cette adresse avec `ClientBuilder::resolve`. Même famille IPv4/IPv6, donc même IP client vue par Vaultwarden. Testé : approbation réelle OK.

Smoke tests (Termux, Vaultwarden réel, dossiers XDG isolés, faux pinentry) : approbation en 2 min 30 puis en 76 s (approbation tardive, hub maintenu), 12 entrées déchiffrées, pinentry fermé par SIGTERM, journal propre ; repli hors ligne (3 essais intacts) ; option désactivée = aucun appel.

**Limites connues** :
- Cloud Bitwarden non testé (hub anonyme, ping SignalR).
- Si le pinentry échoue (pas de tty), l'unlock échoue sans attendre l'approbation.
- Une demande part à chaque déverrouillage, même si l'utilisateur tape son mot de passe (notification sur le téléphone, demande en attente 15 min). Alternative discutée : `CONFIRM` pinentry à deux boutons avant d'envoyer la demande ; reportée après usage réel.

## Suite

1. Usage quotidien. Réinstaller après chaque build : `install -m 755 target/release/rbw target/release/rbw-agent "$XDG_BIN_HOME"/`. Binaires précompilés publiés en release du fork (voir `binaires_precompiles_de_rbw.fr.md`) ; jamais de binaire dans git.
2. PR upstream : branche depuis `main`, sans #378 ni `.agents/`, avec la description du protocole et des limites.

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
