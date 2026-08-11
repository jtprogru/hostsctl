
## The same information elsewhere

`hostsctl --help` prints the command list, `hostsctl <command> --help` narrows it to one
command, and `hostsctl man` writes a man page in roff format:

```bash
hostsctl man > /usr/local/share/man/man1/hostsctl.1
```

Two commands are deliberately absent from the list above. `hostsctl docs cli` and
`hostsctl docs exit-codes` print the raw markdown this page is assembled from; they exist
for `make gen` and are hidden from `--help`.

## Which of these need root

Writing to `/etc/hosts` or to the backup directory needs root, and nothing else does — see
[Permissions and sudo](/hostsctl/guides/permissions/) for the full split and for what
happens to file ownership under `sudo`.
