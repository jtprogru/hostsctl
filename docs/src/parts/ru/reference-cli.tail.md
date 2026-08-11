
## То же самое в других местах

`hostsctl --help` печатает список команд, `hostsctl <команда> --help` сужает его до одной
команды, а `hostsctl man` выдаёт man-страницу в формате roff:

```bash
hostsctl man > /usr/local/share/man/man1/hostsctl.1
```

Двух команд в списке выше намеренно нет. `hostsctl docs cli` и `hostsctl docs exit-codes`
печатают сырой markdown, из которого собирается эта страница; они существуют ради
`make gen` и спрятаны из `--help`.

## Кому из них нужен root

Root нужен для записи в `/etc/hosts` и в каталог бэкапов, и больше ни для чего — полное
разделение и то, что происходит с владельцем файлов под `sudo`, описано в
[Правах и sudo](/hostsctl/ru/guides/permissions/).
