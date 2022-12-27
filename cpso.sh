AdDir="yasea"
jniDir="$AdDir/library/src/main/jniLibs"
RsDir="remote_cam"
BinDir="debug"
if [ $# -ge 1 ] && [ $1 == '-r' ]; then
	BinDir="release"
fi
if [ ! -d "$jniDir" ]; then
        mkdir "$jniDir"
fi
if [ ! -d "$jniDir/arm64-v8a" ]; then 
	mkdir "$jniDir/arm64-v8a"
fi
if [ ! -d "$jniDir/armeabi-v7a" ]; then 
	mkdir "$jniDir/armeabi-v7a"
fi
if [ ! -d "$jniDir/x86" ]; then 
	mkdir "$jniDir/x86"
fi
echo $BinDir
cp $RsDir/target/aarch64-linux-android/$BinDir/lib*.so $jniDir/arm64-v8a/
cp $RsDir/target/armv7-linux-androideabi/$BinDir/lib*.so $jniDir/armeabi-v7a/
cp $RsDir/target/i686-linux-android/$BinDir/lib*.so $jniDir/x86/
