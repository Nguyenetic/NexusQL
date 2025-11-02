# POSIX Compatibility Solutions for Windows

## Overview

This document compares different approaches to get POSIX functions working on Windows for libsql.

## Solution Comparison

| Solution | Difficulty | Completeness | Best For |
|----------|-----------|--------------|----------|
| libunistd | Easy | Good | Quick integration |
| pthreads4w | Medium | Excellent (threads) | Thread-heavy apps |
| MSYS2 | Easy | Complete | Building from source |
| Cygwin | Medium | Complete | Full POSIX environment |

## Detailed Solutions

### 1. libunistd ⭐ RECOMMENDED

**Repository:** https://github.com/robinrowe/libunistd

**What it provides:**
- pthread.h (threading)
- unistd.h (file I/O, system calls)
- sys/socket.h (networking)
- sys/mman.h (memory mapping)
- And many more POSIX headers

**Limitations:**
- No fork() support (not needed for most apps)

**Setup:**
```bash
git clone https://github.com/robinrowe/libunistd.git
# Add -I libunistd to your compile flags
```

---

### 2. pthreads4w (pthreads-win32)

**Repository:** https://sourceforge.net/projects/pthreads4w/

**What it provides:**
- Full POSIX threads API
- Very mature and stable
- Used by many production applications

**Limitations:**
- Only provides pthread - need other libraries for unistd, etc.

**Setup:**
```bash
# Download from SourceForge or
git clone https://github.com/GerHobbelt/pthread-win32.git
# Build and link pthreadVC3.lib
```

---

### 3. MSYS2 with MinGW-w64

**Website:** https://www.msys2.org

**What it provides:**
- Complete POSIX environment
- winpthreads implementation
- Full GNU toolchain
- Package manager (pacman)

**Setup:**
```bash
# Install MSYS2, then:
pacman -S mingw-w64-x86_64-gcc
pacman -S mingw-w64-x86_64-rust  # For Rust projects
```

---

### 4. Windows Subsystem for Linux (WSL)

**What it provides:**
- Complete Linux environment
- Native POSIX support
- Best compatibility

**Limitations:**
- Produces Linux binaries, not Windows .exe
- Requires WSL2 installation

**Use case:** Development and testing, not distribution

---

## Recommendation for libsql

For libsql-experimental on Windows, use **libunistd** because:

1. ✅ Lightweight - mostly headers
2. ✅ Works with both MSVC and MinGW
3. ✅ Provides all needed POSIX functions
4. ✅ No complex build process
5. ✅ Active maintenance

Alternative: If you're building from source anyway, use **MSYS2** for the most complete environment.

## Next Steps

1. Choose your solution based on your build environment
2. See SETUP.md for step-by-step instructions
3. Test with examples/test_pthread.c
4. Integrate with your libsql build
