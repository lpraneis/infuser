# infuser
> It filters your `tee`

`infuser` is meant to replace [tee](https://man7.org/linux/man-pages/man1/tee.1.html) + `grep` to allow for a filter to be changed during execution

It is meant to replace doing the following to forward important stdout lines to a second terminal.
I often find myself using this type of command while running a daemon that spews logs to the terminal to filter
the view in a second terminal pane.
```
some_long_running_command | tee >(rg "important text" > /dev/pts/X)
```

Instead, infuser is used to redirect `stdin` 
```
some_long_running_command | infuser /dev/pts/X "important text"
```

However, the filter used can also be updated during the execution of the program to allow the lines being redirected
to the other terminal tty to change:
```
infuser update "different stuff"
```

## Usage
```
USAGE:
    infuser [OPTIONS] <SUBCOMMAND>

OPTIONS:
    -h, --help                     Print help information
        --sock-name <SOCK_NAME>    Name of unix domain socket to be created in /tmp for IPC
                                   [default: infuser.sock]
    -V, --version                  Print version information

SUBCOMMANDS:
    clear     clear running filter
    help      Print this message or the help of the given subcommand(s)
    run       run and get input
    update    update running infuser
```

### infuser run 
```
run and get input

USAGE:
    infuser run <TTY> [INITAL_FILTER]

ARGS:
    <TTY>              TTY to send filtered lines to
    <INITAL_FILTER>    initial filter

OPTIONS:
    -h, --help    Print help information
```
### infuser update
```
infuser-update 
update running infuser

USAGE:
    infuser update <NEW_FILTER>

ARGS:
    <NEW_FILTER>    updated filter

OPTIONS:
    -h, --help    Print help information
```

### infuser clear
```
clear running filter

USAGE:
    infuser clear

OPTIONS:
    -h, --help    Print help information
```
