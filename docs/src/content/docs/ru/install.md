---
title: Установка
description: Homebrew, crates.io, скрипт установки, архив релиза или сборка из исходников.
sidebar:
  order: 2
---

hostsctl — один статический бинарь без внешних зависимостей. Выбери способ, который
подходит твоей машине.

## Homebrew

```bash
brew install jtprogru/tap/hostsctl
```

Покрывает macOS на Apple silicon и Intel, Linux на `x86_64` и `arm64`. Man-страница и
автодополнения ставятся вместе с формулой.

## crates.io

```bash
cargo install hostsctl
```

Собирает из исходников, поэтому нужен Rust-тулчейн не ниже MSRV (см. `rust-version` в
`Cargo.toml`). Автодополнения и man при этом не ставятся — сгенерируй сам, если нужны:

```bash
hostsctl completions zsh > ~/.zsh/completions/_hostsctl
hostsctl man > /usr/local/share/man/man1/hostsctl.1
```

## Скрипт установки

```bash
curl -fsSL https://raw.githubusercontent.com/jtprogru/hostsctl/main/scripts/install.sh | sh
```

POSIX `sh`, поэтому работает и в alpine-контейнере, где единственный шелл — `ash`. Скрипт
определяет ОС и архитектуру, берёт musl-сборку при отсутствии glibc-загрузчика, проверяет
архив против `checksums.txt` релиза **до** распаковки и ставит бинарь в `/usr/local/bin`.

```bash
# зафиксировать версию и поставить в другое место
curl -fsSL .../install.sh | sh -s -- --version v0.1.0 --bin-dir ~/.local/bin
```

## Архив релиза

Каждый релиз выкладывает `.tar.gz` на каждый таргет на
[странице релизов](https://github.com/jtprogru/hostsctl/releases):

| Таргет | Для чего |
| --- | --- |
| `aarch64-apple-darwin` | macOS, Apple silicon |
| `x86_64-apple-darwin` | macOS, Intel |
| `x86_64-unknown-linux-gnu` | Linux, glibc, x86_64 |
| `aarch64-unknown-linux-gnu` | Linux, glibc, arm64 |
| `x86_64-unknown-linux-musl` | Alpine и другие musl-системы, x86_64 |
| `aarch64-unknown-linux-musl` | Alpine и другие musl-системы, arm64 |

В архиве лежат бинарь, `completions/`, `man/`, README и лицензия.

## Проверка загрузки

Каждый архив перечислен в `checksums.txt`, подписан keyless-подписью
[cosign](https://docs.sigstore.dev/) (`.bundle` рядом) и снабжён SLSA-аттестацией
провенанса сборки.

```bash
# контрольная сумма
sha256sum -c checksums.txt --ignore-missing

# подпись
cosign verify-blob \
  --bundle hostsctl-aarch64-apple-darwin.tar.gz.bundle \
  --certificate-identity-regexp 'https://github.com/jtprogru/hostsctl/.*' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  hostsctl-aarch64-apple-darwin.tar.gz

# провенанс
gh attestation verify hostsctl-aarch64-apple-darwin.tar.gz --repo jtprogru/hostsctl
```

## Из исходников

```bash
git clone https://github.com/jtprogru/hostsctl
cd hostsctl
make install            # соберёт release и положит в /usr/local/bin
```

`make install PREFIX=~/.local` поставит в другое место, `make uninstall` удалит.

## Автодополнения

```bash
hostsctl completions bash
hostsctl completions zsh
hostsctl completions fish
hostsctl completions elvish
hostsctl completions powershell
```

Положи вывод туда, где твой шелл его ищет — для zsh это каталог из `$fpath`, а файл должен
называться `_hostsctl`.
