# Cadatomic

## Getting started

```bash
git clone --recurse-submodules git@github.com:akiselev/cadatomic.git
cd cadatomic/
```

### Setting up Submodules

```bash
cd libs/svelte-nodegui
git remote add upstream git@github.com:nodegui/svelte-nodegui.git
git remote set-url --push upstream DISABLE
cd ../..

cd libs/react-nodegui
git remote add upstream git@github.com:nodegui/react-nodegui.git
git remote set-url --push upstream DISABLE
cd ../..

cd libs/nodegui
git remote add upstream git@github.com:nodegui/nodegui.git
git remote set-url --push upstream DISABLE
cd ../..
```
