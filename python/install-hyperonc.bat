@echo off
set HYPERONC_URL=https://github.com/trueagi-io/hyperon-experimental.git
set HYPERONC_REV=main
set CIBW_BUILD=cp310-win_amd64

set python_url=https://www.python.org/ftp/python/3.12.9/python-3.12.9-amd64.exe

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

set CARGO_HOME=%USERPROFILE%\\.cargo
set RUSTUP_HOME=%USERPROFILE%\\.rustup
curl --proto "=https" --tlsv1.2 -sSf https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe > %TEMP%/rustup-init.exe
call %TEMP%/rustup-init.exe -y
del %TEMP%\rustup-init.exe
set PATH=%PATH%;%USERPROFILE%\\.cargo\\bin
cargo install cbindgen

python -m pip install cmake==3.24 conan==2.19.1 pip==23.1.2
set PATH=%PATH%;%USERPROFILE%\\.local\\bin
conan profile detect --force

rem protobuf-compiler (v3) is required by Das
set PROTOC_ZIP=protoc-31.1-win64.zip
curl -OL https://github.com/protocolbuffers/protobuf/releases/download/v31.1/%PROTOC_ZIP%
mkdir %USERPROFILE%\.local 
tar -xf %PROTOC_ZIP% -C %USERPROFILE%\.local
del -f %PROTOC_ZIP%

mkdir %USERPROFILE%\hyperonc
cd %USERPROFILE%\hyperonc
git init
git remote add origin %HYPERONC_URL%
git fetch --depth=1 origin %HYPERONC_REV%
git reset --hard FETCH_HEAD

mkdir %USERPROFILE%\hyperonc\c\build
cd %USERPROFILE%\hyperonc\c\build

set CMAKE_ARGS=-DBUILD_SHARED_LIBS=ON -DCMAKE_CONFIGURATION_TYPES=Release -DCMAKE_PROJECT_TOP_LEVEL_INCLUDES=%USERPROFILE%/hyperonc/conan_provider.cmake
echo hyperonc CMake arguments: %CMAKE_ARGS%
echo current dir %cd%
cmake %CMAKE_ARGS% ..
cmake --build . --config Release
cmake --build . --target check --config Release