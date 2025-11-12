@echo off
set HYPERONC_URL=https://github.com/DaddyWesker/hyperon-experimental.git
set HYPERONC_REV=release-win

:loop
IF NOT "%1"=="" (
    IF "%1"=="-u" (
        SET HYPERONC_URL=%2
        SHIFT
    )
    IF "%1"=="-r" (
        SET HYPERONC_REV=%2
        SHIFT
    )
    IF "%1"=="?" (
        echo Usage: %~nx0 [-u hyperonc_repo_url] [-r hyperonc_revision]
        echo -u hyperonc_repo_url    Git repo URL to get hyperonc source code
        echo -r hyperonc_revision    Revision of hyperonc to get from Git
        exit /b
    )
    IF "%1"=="-h" (
        echo Usage: %~nx0 [-u hyperonc_repo_url] [-r hyperonc_revision]
        echo -u hyperonc_repo_url    Git repo URL to get hyperonc source code
        echo -r hyperonc_revision    Revision of hyperonc to get from Git
        exit /b
    )
    SHIFT
    GOTO :loop
)

echo hyperonc repository URL: %HYPERONC_URL%
echo hyperonc revision: %HYPERONC_REV%

echo RUNNER_TEMP env: %RUNNER_TEMP%

set CARGO_HOME=%RUNNER_TEMP%\\.cargo
set RUSTUP_HOME=%RUNNER_TEMP%\\.rustup
curl --proto "=https" --tlsv1.2 -sSf https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe > %TEMP%/rustup-init.exe
call %TEMP%/rustup-init.exe -y
del %TEMP%\rustup-init.exe
set PATH=%PATH%;%RUNNER_TEMP%\\.cargo\\bin
cargo install cbindgen

python -m pip install cmake==3.24 conan==2.19.1 pip==23.1.2
set PATH=%PATH%;%RUNNER_TEMP%\\.local\\bin
conan profile detect --force

rem protobuf-compiler (v3) is required by Das
set PROTOC_ZIP=protoc-31.1-win64.zip
curl -OL https://github.com/protocolbuffers/protobuf/releases/download/v31.1/%PROTOC_ZIP%
mkdir %RUNNER_TEMP%\.local 
echo Protoc_zip: %PROTOC_ZIP%
echo Current dir: %cd%
tar -xf %PROTOC_ZIP% -C %RUNNER_TEMP%\.local
del -f %PROTOC_ZIP%

mkdir %RUNNER_TEMP%\\hyperonc
cd %RUNNER_TEMP%\\hyperonc
echo Current dir (should be hyperonc): %cd%
git init
git remote add origin %HYPERONC_URL%
git fetch --depth=1 origin %HYPERONC_REV%
git reset --hard FETCH_HEAD

mkdir %RUNNER_TEMP%\hyperonc\c\build
cd %RUNNER_TEMP%\hyperonc\c\build
echo Current dir (should be hyperonc/c/build): %cd%

set CMAKE_ARGS=-DBUILD_SHARED_LIBS=ON -DCMAKE_CONFIGURATION_TYPES=Release -DCMAKE_PROJECT_TOP_LEVEL_INCLUDES=%RUNNER_TEMP%/hyperonc/conan_provider.cmake -DCMAKE_INSTALL_PREFIX=%RUNNER_TEMP%\\.local
echo hyperonc CMake arguments: %CMAKE_ARGS%
cmake %CMAKE_ARGS% ..
cmake --build . --config Release
cmake --build . --target check --config Release
cmake --build . --target install --config Release