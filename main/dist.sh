rm -rf "dist"
mkdir "dist"
cp -r "$CEF_ROOT"/Release/*.dll "dist"
cp -r "$CEF_ROOT"/Release/*.bin "dist"
cp -r "$CEF_ROOT"/Release/swiftshader "dist"
cp -r "$CEF_ROOT"/Resources/icudtl.dat "dist"

cargo build --release
cp "target/release/main.exe" dist

yarn --cwd "../render" build
cp -r "../render/dist" "dist/app"
