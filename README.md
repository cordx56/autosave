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

autosave watches file changes and automatically saves changed files to the local Git repository as a commit.
By default, changes are saved as `autosave commit` to the `tmp/autosave` branch.

> [!WARNING]
> The author is not responsible for any data loss.
> Please backup your local changes to remote repository frequently.
> If you find any bugs, please report them as an issue.

## Features

- Watches file changes and automatically commits them
- Runs as a background daemon
- Supports multiple repositories simultaneously
- Provides sandbox branches for concurrent development

Supported:

- Linux: Full features
- macOS: Sandbox not supported

## Install

Usually, you can't install autosave by executing only `cargo install autosave` because autosave uses dynamically linked libraries.

Please use the install script:

```bash
curl -L https://github.com/cordx56/autosave/releases/latest/download/install.sh | sh
```

Or, to build from source:

```bash
git clone https://github.com/cordx56/autosave.git
cd autosave
./install.sh
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
# in one terminal
autosave run terminal-1 claude "Implement API"

# in another terminal
autosave run terminal-2 claude "Implement UI"
```

## Uninstall

To uninstall autosave, remove the binary:

```bash
rm $(which autosave)
```

## License

Copyright (C) 2023-2026 cordx56

This software is distributed under the MPL 2.0 license.
