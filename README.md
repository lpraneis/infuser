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
some_long_running_command | infuser run -f "important text" /dev/pts/X
```

However, the filter used can also be updated during the execution of the program to allow the lines being redirected
to the other terminal tty to change:
```
infuser update "different stuff"
```

The tty used can also be changed later, and new output will be sent to the new tty
```
infuser listen
```

## Usage
```
Usage: infuser [OPTIONS] <COMMAND>

Commands:
  clear       clear running filter
  get-filter  get currently running filter
  get-tty     get currently registered tty
  listen      register current tty for output; replaces previous tty, if any
  run         run and get input
  update      update running infuser
  help        Print this message or the help of the given subcommand(s)

Options:
      --sock-name <SOCK_NAME>  Name of unix domain socket to be created in /tmp for IPC [default: infuser.sock]
  -h, --help                   Print help information (use `--help` for more detail)
  -V, --version                Print version information
```
