#!/bin/bash

set -e

mkdir -p .llvm
cd .llvm

rm -rf llvm llvm-project

git clone https://github.com/llvm/llvm-project.git
cd llvm-project
git checkout llvmorg-21.1.0-rc3

rm -rf build
mkdir -p build

cmake -S llvm -B build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release \
  -DBUILD_SHARED_LIBS=OFF \
  -DLLVM_ENABLE_PROJECTS="clang;clang-tools-extra;mlir;compiler-rt" \
  -DLLVM_TARGETS_TO_BUILD="X86" \
  -DLLVM_BUILD_TESTS=OFF \
  -DLLVM_INCLUDE_TESTS=OFF \
  -DLLVM_BUILD_EXAMPLES=OFF \
  -DLLVM_INCLUDE_EXAMPLES=OFF \
  -DLLVM_BUILD_DOCS=OFF \
  -DLLVM_ENABLE_DOXYGEN=OFF \
  -DLLVM_ENABLE_LTO=OFF \
  -DLLVM_ENABLE_SPHINX=OFF \
  -DLLVM_STATIC_LINK_CXX_STDLIB=ON \
  -DLLVM_ENABLE_ZLIB=OFF \
  -DLLVM_ENABLE_LIBXML2=OFF \
  -DLLVM_ENABLE_LIBEDIT=OFF \
  -DCMAKE_INSTALL_PREFIX=../llvm

ninja -C build install

cd ../llvm
mv bin/llvm-config .
rm bin/*
mv llvm-config bin/
cd ..
tar -cJf linux-x64.tar.xz llvm

echo "LLVM build and packaging completed successfully!"
