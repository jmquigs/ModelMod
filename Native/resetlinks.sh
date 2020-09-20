set -e 

DIRS="hook_core shared_dx9 d3dx constant_tracking global_state input profiler types util device_state dnclr mod_load interop"

WD=$(pwd)
for d in $DIRS; do 
    cd $WD/$d
    echo "Resetting link in $WD/$d"
    rm -rf ./target 
    sh ../setjunc.sh ModelMod
done