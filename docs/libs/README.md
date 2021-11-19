# Vendored Libraries

Cadatomic uses a variety of customized vendored libraries like `svelte-nodegui`.

1. [Create a private fork](private_fork.md)
2. [Add a git submodule](https://git-scm.com/book/en/v2/Git-Tools-Submodules)


# Removing git submodule

```sh
# Remove the submodule entry from .git/config
git submodule deinit -f path/to/submodule

# Remove the submodule directory from the superproject's .git/modules directory
rm -rf .git/modules/path/to/submodule

# Remove the entry in .gitmodules and remove the submodule directory located at path/to/submodule
git rm -f path/to/submodule
```