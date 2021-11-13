# Building GTK

See instructions in GTK docs for [building GLib](https://docs.gtk.org/glib/building.html) and [building GTK](https://docs.gtk.org/gtk4/building.html).

## Building Glib

```
meson setup --prefix "$HOME/svelters/zig/lib/glib" --buildtype debug builddir
meson compile -C builddir
meson install -C builddir

```

## Building GTK4

```
SV="$HOME/svelters/zig/lib/glib"
CPPFLAGS="-I$SV/include"
LDFLAGS="-L$SV/lib"
PKG_CONFIG_PATH="$SV/lib/pkgconfig"
export CPPFLAGS LDFLAGS PKG_CONFIG_PATH

meson setup --prefix $HOME/svelters/zig/lib/gtk --buildtype debug builddir
cd builddir
ninja
ninja install

```

## Running Svelters

```sh
# SV="$HOME/svelters/zig/lib/"
# CPPFLAGS="-I$SV/gtk/include -I$SV/glib/include -I$SV/glib/lib/glib-2.0/include"
# LDFLAGS="-L$SV/gtk/lib -L$SV/glib/lib"
# PKG_CONFIG_PATH="$SV/glib/lib/pkgconfig:$SV/gtk/lib/pkgconfig"

# export LD_LIBRARY_PATH="$SV/glib/lib:$SV/gtk/lib:$LD_LIBRARY_PATH"

export LD_LIBRARY_PATH="$HOME/svelters/zig/lib/glib/lib:$HOME/svelters/zig/lib/gtk/lib"
export PKG_CONFIG_PATH="$HOME/svelters/zig/lib/glib/lib/pkgconfig:$HOME/svelters/zig/lib/gtk/lib/pkgconfig:$PKG_CONFIG_PATH"
```