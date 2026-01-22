<div align="center">
    <h1>
        💾
        <br>
        autosave
    </h1>
    <p>
        Automatically save all your changes to the repository
    </p>
</div>

autosave watches for file changes and automatically saves changed files to the local Git repository as a commit.
By default, changes are committed with the message `autosave commit` to the `tmp/autosave` branch.

> [!WARNING]
> The author is not responsible for any data loss.
> Please back up your local changes to remote repository frequently.
> If you find any bugs, please report them as an issue.

## Features

- Watches file changes and automatically commits them
- Runs as a background daemon
- Supports multiple repositories simultaneously
- Provides sandbox branches for concurrent development

Supported: Linux, macOS

## Install

```bash
curl -L https://github.com/cordx56/autosave/releases/latest/download/install.sh | sh
```

Or, build from source:

```bash
cargo install autosave --locked
```

## Usage

To watch changes and save the current repository automatically:

```bash
autosave
```

Once you add the repository, it will be watched until it is removed.

To list the current watch list:

```bash
autosave list
```

To remove the current repository from the watch list:

```bash
autosave remove
```

To stop the daemon:

```bash
autosave kill
```

### Start Daemon Automatically

Add the following line to your shell rc file (e.g. `.bashrc`, `.zshrc`) to start the `autosave` daemon automatically:

```bash
(command -v autosave && autosave list) > /dev/null
```

### Sandbox

If you would like to enter a sandbox Git branch, you can use:

```bash
autosave run [branch name]
```

This allows you to make file changes on the specified branch without affecting your current branch.
All changes are automatically committed to the sandbox branch, keeping your original working directory clean.

For example, you can run multiple development sessions concurrently on different branches:

```bash
# this starts $SHELL in the Git sandbox
autosave run shell

# in one terminal
autosave run terminal-1 claude "Implement API"

# in another terminal
autosave run terminal-2 claude "Implement UI"
```

After exiting the process, you can merge your sandboxed branch.

## Uninstall

To uninstall autosave, remove the binary:

```bash
rm "$(which autosave)"
```

## License

Copyright (C) 2023-2026 cordx56

This software is distributed under the MPL 2.0 license.
