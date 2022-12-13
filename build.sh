BinDir = ""
if [ $# -ge 1 ] && [ $1 == '-r' ]; then
	BinDir="--release"
fi
cd remote_cam
cargo build --target i686-linux-android $BinDir
cargo build --target aarch64-linux-android $BinDir