set -e 

rustoutroot=$1

thistarget="./target"

if [[ -L "$thistarget" && -d "$thistarget" ]]; then 
    echo "$thistarget is already a symlink"
    exit 0
fi

rustout=""
if [ -d "/f" ]; then 
    rustout="/f/RustOut"
elif [ -d "/e" ]; then
    rustout="/e/RustOut"
fi

if [ "$rustoutroot" != "" ]; then 
    rustout="$rustout/$rustoutroot"
fi

if [ "$rustout" == "" ]; then 
    echo "Can't find suitable rust out directory"
    exit 1
fi

dir=$(basename $(pwd))
rustout="$rustout/$dir"
echo $rustout

if [ ! -d $rustout ]; then 
    mkdir -p $rustout
fi

target="$rustout/target"
if [ ! -d $target ]; then 
    if [ -d "./target" ]; then 
        echo "moving target: ./target => $target"
        mv "./target" $target
    fi
fi

if [ ! -d "$target" ]; then 
    mkdir -p $target
fi

dir=$(pwd)
thistarget="$dir/target"
# cygpath will fail if the dir doesn't exist, so make it
made_it=false
if [ ! -d $thistarget ]; then 
    touch $thistarget
    made_it=true
fi
dos_thistarget=$(cygpath -d $thistarget)
if [ "$made_it" == "true" ]; then 
    rm $thistarget
fi

dos_desttarget=$(cygpath -d $target)
echo "mklink /j $dos_thistarget $dos_desttarget" > mklink.bat
cmd <mklink.bat

rm mklink.bat