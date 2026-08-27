# `pina completions`

Generate a completion script from the authoritative Clap command tree.

```text
pina completions <SHELL>
```

Supported shells are Bash, Elvish, Fish, PowerShell, and Zsh. The script is written to stdout without progress text:

```bash
pina completions bash > ~/.local/share/bash-completion/completions/pina
pina completions zsh > ~/.zfunc/_pina
pina completions fish > ~/.config/fish/completions/pina.fish
```

Regenerate the script after upgrading Pina so new commands, options, and help text are available to the shell.
