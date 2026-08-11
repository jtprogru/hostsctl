---
title: Как участвовать
description: Как собрать, протестировать и предложить изменение.
---

Багрепорты, запросы фич, правки документации и код — всё уместно. Полная версия этой
страницы лежит в
[CONTRIBUTING.md](https://github.com/jtprogru/hostsctl/blob/main/CONTRIBUTING.md).

## Настройка

Нужен Rust-тулчейн не ниже MSRV, объявленной как `rust-version` в `Cargo.toml`.

```bash
git clone https://github.com/jtprogru/hostsctl
cd hostsctl
make            # покажет все цели
make build
make test
```

Для линтеров и релизных помощников:

```bash
make install-tools    # shellcheck, shfmt, actionlint, cargo-deny, cargo-audit
```

## Перед пуллреквестом

```bash
make ci     # fmt-check, clippy, shellcheck, actionlint, тесты, gen-check, msrv
```

Это тот же набор и в том же порядке, что гоняет CI. Две цели стоит пояснить:

- `gen-check` перегенерирует `docs/src/generated/` из бинаря и падает, если закоммиченная
  копия отличается. Тронул определение CLI — запусти `make gen` и закоммить результат.
- `msrv` собирает на минимальной поддерживаемой версии Rust, которая обычно старше твоего
  `stable`.

## Работа с сайтом документации

```bash
make docs-install
make docs-dev        # http://localhost:4321/hostsctl/
make docs-build
```

Английский — основной язык, он лежит в `docs/src/content/docs/`; русская локаль зеркалит
его в `docs/src/content/docs/ru/`. Отсутствующая русская страница падает на английский
оригинал, а не в 404, поэтому неполный перевод допустим — английская страница без русской
пары сборку не ломает.

## Соглашения

- Коммиты по [Conventional Commits](https://www.conventionalcommits.org/):
  `feat(zones): ...`, `fix: ...`, `docs: ...`.
- Ветки: `feature/<short-desc>`, `fix/<short-desc>`, `docs/<short-desc>`.
- Пользовательские строки и публичная документация — на английском. Внутренние комментарии
  в Rust-исходниках на русском; следуй файлу, который правишь, а не переводи его.
- Одно логическое изменение на коммит. Рефакторинг и изменение поведения — разными
  коммитами.

## Тесты

Интеграционные тесты гоняют настоящий бинарь по копии `/etc/hosts` во временном каталоге
через `--target`. Системный файл они не трогают и root им не нужен. Если изменение
потребовало root в тесте — это сигнал, что изменение неправильное.

## Релиз

Только для мейнтейнеров:

```bash
make release-prep VERSION=0.2.0     # проставит Cargo.toml и обновит lockfile
# напиши секцию CHANGELOG.md для 0.2.0, закоммить
make version-check TAG=v0.2.0       # то, что проверит CI
git tag -a v0.2.0 -m "v0.2.0" && git push origin v0.2.0
```

Тег запускает релизный workflow: кросс-сборка на шесть таргетов, контрольные суммы,
keyless-подписи cosign, SLSA-аттестация провенанса, GitHub Release с нотами из changelog,
публикация на crates.io и обновление формулы Homebrew. Тег с дефисом (`v0.2.0-rc1`)
публикуется как pre-release и тап не трогает.
