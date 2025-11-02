# Complete Guide: Building libsql-experimental on Windows

This guide provides step-by-step instructions for building libsql-experimental on Windows with POSIX compatibility.

## Table of Contents
1. [Prerequisites](#prerequisites)
2. [Quick Start (Recommended)](#quick-start-recommended)
3. [Troubleshooting](#troubleshooting)
4. [Performance Optimization](#performance-optimization)

---

## Prerequisites

### Required Tools
- **Rust** (1.70+): Install from https://rustup.rs/
- **Git**: Install from https://git-scm.com/
- **Visual Studio Build Tools** or **MinGW-w64**

### Verify Installation
```bash
rustc --version
cargo --version
git --version
```

---

## Quick Start (Recommended)

**Time: 10-15 minutes**

### Step 1: Clone libsql-experimental
```bash
git clone https://github.com/tursodatabase/libsql.git
cd libsql
```

### Step 2: Install POSIX Compatibility Layer
```bash
cd C:\Users\jnguyen\Documents\Project\POSIX
install_libunistd.bat
```

### Step 3: Configure Build Environment
```cmd
set POSIX_DIR=C:\Users\jnguyen\Documents\Project\POSIX
set CFLAGS=-I%POSIX_DIR%\deps\libunistd
set CXXFLAGS=-I%POSIX_DIR%\deps\libunistd
set BINDGEN_EXTRA_CLANG_ARGS=-I%POSIX_DIR%\deps\libunistd
```

### Step 4: Build libsql
```bash
cd path\to\libsql
cargo build --release
```

### Step 5: Verify Installation
```bash
cargo test
```

---

## Troubleshooting

### Common Build Errors

#### 1. "error: linker link.exe not found"
**Solution:**
```bash
rustup default stable-x86_64-pc-windows-gnu
```

#### 2. "fatal error: pthread.h file not found"
**Solution:**
```bash
cd C:\Users\jnguyen\Documents\Project\POSIX
install_libunistd.bat
set CFLAGS=-IC:\Users\jnguyen\Documents\Project\POSIX\deps\libunistd
```

#### 3. "undefined reference to pthread_create"
**Solution:**
```bash
rustup target add x86_64-pc-windows-gnu
cargo build --target x86_64-pc-windows-gnu --release
```

---

## Performance Optimization

### Enable Link-Time Optimization
Add to `Cargo.toml`:
```toml
[profile.release]
lto = true
codegen-units = 1
opt-level = 3
```

### CPU-Specific Optimizations
```bash
set RUSTFLAGS=-C target-cpu=native
cargo build --release
```

---

## Success Checklist

- [ ] Rust toolchain installed (1.70+)
- [ ] POSIX compatibility layer installed
- [ ] Environment variables configured
- [ ] libsql repository cloned
- [ ] Build completes successfully
- [ ] Tests pass

---

**Last Updated:** 2024-11-02
**Status:** Production Ready
