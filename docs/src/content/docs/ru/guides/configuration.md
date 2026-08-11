---
title: Конфигурация
description: Что лежит в config.yaml, как записи связывают адреса и имена, где утилита ищет файл.
---

Конфиг — источник правды. `/etc/hosts` из него получается, а не наоборот. Исключение —
всё, что вне управляемого блока: это hostsctl не читает обратно и не трогает.

## Где лежит файл

По убыванию приоритета:

1. `--config /path/to/config.yaml`
2. `$HOSTSCTL_CONFIG`
3. `$XDG_CONFIG_HOME/hostsctl/config.yaml`
4. `~/.config/hostsctl/config.yaml`

Под `sudo` `$XDG_CONFIG_HOME` игнорируется — он указывает на окружение root, а не твоё, —
и домашний каталог берётся из `SUDO_USER` через passwd. Подробности на странице
[Права и sudo](/hostsctl/ru/guides/permissions/).

```bash
hostsctl config-path          # конфиг
hostsctl config-path --all    # конфиг и все подключённые зоны
```

## Как он устроен

```yaml
version: 1
settings:
  target: /etc/hosts
  backup_dir: /var/db/hostsctl/backups
  keep_backups: 20
  flush_dns: true
include:
  - zones/*.yaml
  - zones/*.hosts
groups:
  - name: local
    enabled: true
    description: Local development
    entries:
      - ip: 127.0.0.1
        hostnames: [k8s.orb.local]
        enabled: true
        comment: orbstack
```

Каждое поле описано в [справочнике конфига](/hostsctl/ru/reference/config/).

## Запись связывает N адресов и M имён

Запись — это связка адресов и имён. `ip` принимает и скаляр, и список, `hostnames` —
всегда список:

```yaml
- ip: 10.0.0.7                          # один адрес, несколько имён
  hostnames: [api.local, web.local]
- ip: [192.178.194.100, 192.178.194.101, 192.178.194.102]
  hostnames: [analytics.google.com]     # одно имя, несколько адресов
```

В `/etc/hosts` это разворачивается в строку на каждый адрес, и в каждой — весь набор имён.
hostsctl ничего не выбрасывает: схлопывается только точный повтор пары «адрес + имя», и об
этом будет предупреждение. Имя, объявленное в двух группах, тоже не ошибка, но про такое
утилита скажет — обычно это случайность.

## Добавление и удаление

`hostsctl add` доливает адрес к существующей записи, если набор имён совпадает:

```bash
hostsctl add 192.178.194.100 analytics.google.com
hostsctl add 192.178.194.101 analytics.google.com   # станет ip: [.100, .101]
hostsctl rm 192.178.194.101                         # снимет только этот адрес
hostsctl rm analytics.google.com                    # уберёт имя со всеми адресами
```

Асимметрия здесь намеренная: удаление по адресу не должно уносить с собой остальные адреса
имени.

## Выключено — не удалено

```bash
hostsctl disable k8s.orb.local
hostsctl enable  k8s.orb.local
hostsctl list --all               # --all показывает и выключенные
```

Выключенная запись остаётся в конфиге и не попадает в `/etc/hosts`. В `.hosts`-зоне она
пишется закомментированной строкой — так файл остаётся читаемым руками.

## Настройки

| Ключ | По умолчанию | Что делает |
| --- | --- | --- |
| `target` | `/etc/hosts` | Файл, в который рендерится управляемый блок. |
| `backup_dir` | `/var/db/hostsctl/backups` на macOS, `/var/lib/hostsctl/backups` иначе | Куда складывать снимки. |
| `keep_backups` | `20` | Сколько снимков хранить; `0` — не чистить. |
| `flush_dns` | `true` | Сбрасывать DNS-кеш после успешной записи. |

`--target` переопределяет `settings.target` на один запуск — именно так тесты работают с
копией вместо настоящего файла.

## Правка руками

```bash
hostsctl edit                # откроет конфиг в $EDITOR и прогонит check
hostsctl edit work           # откроет файл, в котором лежит группа 'work'
```

`edit` перечитывает конфиг после выхода из редактора и запускает `check`, так что опечатка
всплывает сразу, а не на следующем `apply`.
