# Binaires précompilés de rbw

État au 2026-10-10. Fait : workflow `.github/workflows/release.yaml` sur `kml93`, release `1.15.0-kml93.1` publiée (https://github.com/kml93/rbw/releases/tag/1.15.0-kml93.1).

## Workflow

- Déclencheurs : tag `*-kml93.*` (build + release en **brouillon** avec les archives et `SHA256SUMS`) ou `workflow_dispatch` (build seul, archives en artefacts du run).
- `linux_amd64` : cible `x86_64-unknown-linux-musl`, `clang-18` comme dans `tests.yaml` d'upstream ; le workflow lance `rbw --version` sur le runner.
- `android_arm64` : cible `aarch64-linux-android`, NDK du runner (`$ANDROID_NDK_LATEST_HOME`), API 24, `--no-default-features` (pas de `--clipboard`, comme le paquet Termux). Binaire : `NEEDED` libdl/libm/libc, alignement 16 Ko.
- Runners `ubuntu-24.04`, build ~2 min par cible.

## Conventions choisies avec l'utilisateur

- Tag `<version upstream>-kml93.<N>` sans `v` (comme les tags upstream `1.15.0`) ; `N` compte les builds du fork sur une même version upstream.
- Archives `rbw_<tag>_<os>_<arch>.tar.gz` (style de doy : `rbw_1.15.0_linux_amd64.tar.gz`), contenant `rbw` et `rbw-agent`.
- Description de la release en anglais : fork non relu par upstream, changements, assets, installation par `gh release download`.

## Publier une nouvelle release

1. Montrer le tag et la description à l'utilisateur (`ask`) avant de pousser le tag.
2. `git tag <tag> <commit> && git push origin <tag>` ; attendre le run.
3. `gh release edit <tag> --repo kml93/rbw --notes-file <notes> --draft=false --latest`.

## Vérifications faites (2026-10-10)

- Run du tag `38081456355` : 2 cibles + release vertes.
- Sur ce téléphone : `gh release download` + `sha256sum -c` OK, `rbw --version` → `rbw 1.15.0`.
- Déverrouillage par approbation avec le `rbw-agent` téléchargé (via `RBW_AGENT`, vraie config, agent installé arrêté) : phrase identique dans l'appli, `rbw unlock` code 0, `rbw unlocked` OK. Agent de test arrêté ensuite.

## Reste

- Binaire `linux_amd64` sur le PC Manjaro : seulement testé sur le runner (`rbw --version`).
- Rien n'a été installé sur ce téléphone : les binaires de `$XDG_BIN_HOME` restent ceux compilés localement.
