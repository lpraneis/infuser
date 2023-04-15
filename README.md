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

## Windows Support
`infuser` runs in a slightly different capacity on Windows. 
Instead of specifying a `tty` on the command line, general operation is to start the "listening" process first and then run `infuser listen` in a separate terminal pane. This limitation is due to the fact that named pipes are used on Windows to send output from one terminal to another, rather than writing directly to the unix pts object.

Additionally, `infuser` only works in `cmd.exe` and not Powershell. This is because of how Powershell buffers pipeline output rather than allowing both processes to run concurrently. This will hopefully be fixed in future updates.

## Usage
```
Filters your tee

Usage: infuser [OPTIONS] <COMMAND>

Commands:
  clear       Clear running filter
  get-filter  Get currently running filter
  get-tty     Get currently registered tty or console
  listen      Register current console for output; replaces previous tty or console, if any. This is required on Windows since there aren't ttys
  run         Run and get input
  update      Update running infuser filter
  help        Print this message or the help of the given subcommand(s)

Options:
      --sock-name <SOCK_NAME>  Name of communication pipe On Unix, this is a Unix Domain Socket in /tmp On Windows, this is the name of a named pipe [default: infuser.pipe]
  -h, --help                   Print help information (use `--help` for more detail)
  -V, --version                Print version information
```
