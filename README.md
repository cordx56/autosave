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

autosave watches file changes and automatically saves changed file to local Git repository as a commit.
By default, changes are saved as `autosave commit` to the `tmp/autosave` branch.

## Warning!

The author is not responsible for any data loss.
Please backup your local changes to remote repository frequently.
And if you found any bugs, please report it as an issue.

## Usage

To watch changes and save current repository automatically:

```bash
autosave
```

To list current watch list:

```bash
autosave list
```

To remove path from watch list:

```bash
autosave remove -p [path]
```

Add below line to your shell rc file (e.g. `.bashrc`) to start `autosave` daemon automatically:

```bash
(command -v autosave && autosave list) > /dev/null
```

## Install

```
cargo install autosave --locked
```

## License

Copyright (C) 2023 cordx56

This project is distributed under the MPL 2.0 license.
