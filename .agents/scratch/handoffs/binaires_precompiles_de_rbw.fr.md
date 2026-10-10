# Binaires précompilés de rbw

État au 2026-10-10. Rien n'est fait. La branche `kml93` est poussée sur `origin` (`bd34c7f`), avec `unlock_with_device`, le correctif IP et le merge de doy/rbw#378.

## Objectif

Installer le rbw patché (`kml93`) sur plusieurs machines sans compiler sur chacune : d'autres téléphones Termux (Android aarch64) et un PC Manjaro (Linux x86_64). Un workflow GitHub Actions compile les binaires et les attache à une release du fork `kml93/rbw`.

## Faits

**Upstream (`doy/rbw`)** :
- Aucun workflow de release. Le seul workflow est `.github/workflows/tests.yaml` (tests).
- doy compile ses releases à la main via le `Makefile` : cible `x86_64-unknown-linux-musl` (binaire statique), paquets `rbw_<version>_linux_amd64.tar.gz` et `.deb`.
- La release 1.15.0 (2025-12-31) n'a qu'un fichier : `rbw_1.15.0_linux_amd64.tar.gz`. Pas d'aarch64, pas d'Android, pas de macOS.
- Les distributions (Arch `extra/rbw`, Termux `packages/rbw`) recompilent depuis le tag, sans utiliser ces binaires.

**Limites GitHub** :
- Fichiers suivis par git : refus au-delà de 100 Mo par fichier, avertissement dès 50 Mo. Ne jamais commiter de binaire.
- Fichiers attachés à une release : jusqu'à 2 Gio par fichier, sans compter dans la taille du dépôt. Les binaires font environ 10 Mo (`rbw`) et 14 Mo (`rbw-agent`).
- GitHub Actions est gratuit pour les dépôts publics, y compris les runners Linux arm64 (`ubuntu-24.04-arm`).

**Contraintes de compilation** :
- `ring` (via `rustls`) contient du C : chaque cible exige un compilateur C pour cette cible.
- Android (`aarch64-linux-android`) utilise la libc bionic. La compilation croisée (compiler sur un runner x86_64 pour une autre plateforme) demande le NDK Android : le linker et le clang de la cible, plus une variable `CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER`. Outils possibles : `cargo-ndk`, ou `cross`.
- Termux compile avec `--no-default-features` (`arboard`, le presse-papiers, ne compile pas sur Android). Le niveau d'API Android visé doit être ≤ celui des téléphones (Termux utilise l'API 24).
- Manjaro x86_64 : la cible `x86_64-unknown-linux-musl` (comme doy) donne un binaire statique, qui marche sur toute distribution Linux x86_64 ; elle demande `musl-tools` sur le runner.
- Ne pas mettre `-C target-cpu=native` : sur ce téléphone, il a produit un binaire qui meurt sur SIGILL.

## Plan proposé

1. Workflow `.github/workflows/release.yaml` sur `kml93`, déclenché par un tag `v*-kml93.*` (par exemple `v1.15.0-kml93.1`) ou à la main (`workflow_dispatch`).
2. Matrice de 2 cibles :
   - `x86_64-unknown-linux-musl` sur `ubuntu-latest`, avec `musl-tools` ;
   - `aarch64-linux-android` sur `ubuntu-latest` avec le NDK (`cargo-ndk`), `--no-default-features`.
3. Archive par cible : `rbw_<tag>_<cible>.tar.gz` contenant `rbw` et `rbw-agent`, plus un fichier `SHA256SUMS`.
4. `gh release create` (ou `softprops/action-gh-release`) attache les archives à la release du tag.
5. Installation sur chaque machine : télécharger l'archive (`gh release download`), vérifier `SHA256SUMS`, puis `install -m 755 rbw rbw-agent "$XDG_BIN_HOME"/`. Sur Termux, `$XDG_BIN_HOME` doit passer avant `$PREFIX/bin` dans le `PATH`.

## À décider avec l'utilisateur

- Release publique sur le fork : l'utilisateur est d'accord pour son propre usage. Indiquer clairement dans la description de la release que c'est un fork non relu par upstream.
- Ajouter ou non une cible `aarch64-unknown-linux-musl` (Linux ARM hors Android).
- Script d'installation dans le dépôt, ou une simple commande documentée.

## Vérification attendue

- Le workflow passe sur les 2 cibles.
- Le binaire Android, téléchargé sur ce téléphone, renvoie `rbw --version` et déverrouille par approbation (étape qui demande l'utilisateur : passer par `ask`).
- Le binaire musl renvoie `rbw --version` sur Manjaro (ou, à défaut, dans un conteneur `x86_64`).

## Suite du déverrouillage par appareil

Voir `deverrouillage_biometrique_de_rbw.fr.md` : usage quotidien, puis PR upstream de B (branche depuis `main`, sans #378 ni `.agents/`).
