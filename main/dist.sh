rm -rf "dist"
mkdir "dist"

cp -r "config.yaml" "dist"
cp -r "extern"/* "dist"
mkdir "dist/icons"

cp -r "$CEF_ROOT"/Release/*.dll "dist"
cp -r "$CEF_ROOT"/Release/*.bin "dist"
cp -r "$CEF_ROOT"/Release/swiftshader "dist"
cp -r "$CEF_ROOT"/Resources/icudtl.dat "dist"

cargo build --release
cp "target/release/main.exe" dist

yarn --cwd "../ui" build
cp -r "../ui/dist" "dist/app"
