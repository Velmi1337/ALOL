#!/bin/bash
set -e
export APPDIR="${APPDIR:-$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)}"
if [[ ${ALOL_APPIMAGE_ACTIVE:-} != 1 ]]; then
    # Prefix s preserves an empty value; u preserves an unset variable.
    for alol_name in PATH LD_LIBRARY_PATH LD_PRELOAD XDG_DATA_DIRS \
        GSETTINGS_SCHEMA_DIR GDK_PIXBUF_MODULE_FILE GDK_PIXBUF_MODULEDIR \
        GTK_EXE_PREFIX GTK_DATA_PREFIX GTK_PATH GTK_IM_MODULE_FILE GTK_THEME \
        GI_TYPELIB_PATH GDK_BACKEND GIO_MODULE_DIR GIO_EXTRA_MODULES; do
        if [[ -v $alol_name ]]; then
            export "ALOL_APPIMAGE_HOST_${alol_name}=s${!alol_name}"
        else
            export "ALOL_APPIMAGE_HOST_${alol_name}=u"
        fi
    done
fi
export ALOL_APPIMAGE_ACTIVE=1
export LD_LIBRARY_PATH="$APPDIR/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export PATH="$APPDIR/usr/bin:$PATH"
export GIO_MODULE_DIR="$APPDIR/usr/lib/gio/modules"
export GIO_EXTRA_MODULES="$APPDIR/usr/lib/gio/modules"
for alol_hook in "$APPDIR"/apprun-hooks/*.sh; do
    [[ ! -f $alol_hook ]] || source "$alol_hook"
done
exec "$APPDIR/usr/bin/alol" "$@"
