set -e

# WARNING: this script can remove existing Rust "target" directories and files inside.
# but it shouldn't make any changes unless you run it with DRY_RUN=0 in the environment.
# NOTE: since moving to a workspace this script is less useful since the
# workspace uses a single target directory (at the root).  Notably the
# detection scheme used by this script (looking for lib.rs) does not locate
# that workspace-specific "target" directory, so that symlink (if needed)
# must be created manually.

# I use symlinks for the target directories to redirect Rust output files to a filesystem with
# better performance and more importantly a lot more space.  These symlinks are windows
# "Junction" files created with the mklink tool.  The related script `setjunc.sh` is used to
# make the links.  The output is kind of spammy because a new windows shell is launched for
# each dir.

# This script resets all the symlinks to point at new targets.  It also removes existing target
# directories (if any) that have not been symlinked.  The files in those directories are
# destroyed, so cargo must be re-run to rebuild the rust code.

DIRS=$(ls)
WD=$(pwd)

for d in $DIRS; do
    libsrc="$WD/$d/src/lib.rs"
    if [ -f "$libsrc" ]; then
        echo "Checking lib $WD/$d"
        if [ -d "$WD/$d/target" ]; then
            if [ "$DRY_RUN" == "0" ]; then
                cd $WD/$d
                echo "Recreating link in $WD/$d"
                rm -rf ./target
                sh ../setjunc.sh ModelMod
            fi
                ls "$WD/$d/target/"
                if [ -L "$WD/$d/target" ]; then
                    echo "  Would reset existing link: $WD/$d/target"
                elif [ -d "$WD/$d/target" ]; then
                    echo "  Would REMOVE existing dir and files and recreate link: $WD/$d/target"
                else
                    echo "  Would create new link: $WD/$d/target"
                fi

        fi
    fi
done

if [ "$DRY_RUN" != "0" ]; then
    echo "dry run complete, rerun with DRY_RUN=0 to make changes"
fi