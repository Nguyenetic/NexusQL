# Windows Setup Guide: Building libsql-experimental from Scratch

This guide shows you exactly what to do on a fresh Windows computer to build libsql-experimental successfully.

## Prerequisites (Install These First)

### 1. Install Rust
```bash
# Download and run from: https://rustup.rs/
# Or use winget:
winget install Rustlang.Rustup

# Verify installation:
rustc --version
cargo --version
```

### 2. Install Git
```bash
# Download from: https://git-scm.com/
# Or use winget:
winget install Git.Git

# Verify:
git --version
```

### 3. Install uv (Python Package Manager)
```bash
# Download from: https://github.com/astral-sh/uv
# Or use PowerShell:
powershell -c "irm https://astral.sh/uv/install.ps1 | iex"

# Verify:
uv --version
```

---

## Step-by-Step Setup

### Step 1: Clone the POSIX Compatibility Repository

```bash
# Clone your POSIX repository (adjust URL as needed)
git clone <your-posix-repo-url> POSIX
cd POSIX
```

### Step 2: Run Automated Setup

```bash
# This installs libunistd POSIX compatibility layer
setup_libsql_build.bat
```

**What this does:**
- Checks prerequisites (Rust, Cargo, Git)
- Clones libunistd into `deps/libunistd`
- Sets up environment variables
- Creates a build helper script

---

## Building libsql-experimental

### Option A: From PyPI Package (If Downloaded)

If you have the libsql-experimental source:

```bash
# 1. Navigate to the directory
cd path\to\libsql_experimental-0.0.55

# 2. Create Python virtual environment with uv
uv venv --python 3.13

# 3. Activate virtual environment
.venv\Scripts\activate

# 4. Install maturin (Rust-Python build tool)
uv pip install maturin

# 5. Set POSIX environment variables
set POSIX_DIR=C:\path\to\POSIX
set CFLAGS=-I%POSIX_DIR%\deps\libunistd
set CXXFLAGS=-I%POSIX_DIR%\deps\libunistd
set BINDGEN_EXTRA_CLANG_ARGS=-I%POSIX_DIR%\deps\libunistd

# 6. Build with maturin
maturin build --release

# 7. Install the built wheel
uv pip install target/wheels/libsql_experimental-*.whl

# 8. Test it works
python -c "import libsql_experimental; print('Success!')"
```

### Option B: From GitHub Repository

If building from libsql source:

```bash
# 1. Clone libsql
git clone https://github.com/tursodatabase/libsql.git
cd libsql

# 2. Navigate to Python bindings (if separate)
# Or use the main directory if it contains Cargo.toml with pyo3

# 3. Follow steps 2-8 from Option A above
```

---

## Quick Reference: Environment Variables

For building libsql-experimental, you need these environment variables set:

```batch
set POSIX_DIR=C:\path\to\POSIX
set CFLAGS=-I%POSIX_DIR%\deps\libunistd
set CXXFLAGS=-I%POSIX_DIR%\deps\libunistd
set BINDGEN_EXTRA_CLANG_ARGS=-I%POSIX_DIR%\deps\libunistd
```

### Save as a batch file (optional):

Create `set_posix_env.bat`:

```batch
@echo off
set POSIX_DIR=C:\Users\%USERNAME%\Documents\Project\POSIX
set CFLAGS=-I%POSIX_DIR%\deps\libunistd
set CXXFLAGS=-I%POSIX_DIR%\deps\libunistd
set BINDGEN_EXTRA_CLANG_ARGS=-I%POSIX_DIR%\deps\libunistd
echo POSIX environment variables configured!
```

Then just run: `call set_posix_env.bat` before building.

---

## Complete Build Script (All-in-One)

Create `build_libsql_windows.bat`:

```batch
@echo off
echo ========================================
echo Building libsql-experimental for Windows
echo ========================================
echo.

REM 1. Set POSIX paths
set POSIX_DIR=C:\path\to\POSIX
set CFLAGS=-I%POSIX_DIR%\deps\libunistd
set CXXFLAGS=-I%POSIX_DIR%\deps\libunistd
set BINDGEN_EXTRA_CLANG_ARGS=-I%POSIX_DIR%\deps\libunistd

REM 2. Create virtual environment if it doesn't exist
if not exist .venv (
    echo Creating virtual environment...
    uv venv --python 3.13
)

REM 3. Activate venv and install maturin
call .venv\Scripts\activate.bat
uv pip install maturin

REM 4. Build
echo Building with maturin...
maturin build --release

REM 5. Install
echo Installing built wheel...
for %%f in (target\wheels\*.whl) do uv pip install %%f

REM 6. Test
echo Testing import...
python -c "import libsql_experimental; print('SUCCESS! Version:', libsql_experimental.__version__ if hasattr(libsql_experimental, '__version__') else '0.0.55')"

echo.
echo ========================================
echo Build complete!
echo ========================================
pause
```

---

## Troubleshooting

### Error: "pthread.h not found"

```bash
# Verify libunistd is installed
dir C:\path\to\POSIX\deps\libunistd

# If not, reinstall:
cd C:\path\to\POSIX
setup_libsql_build.bat
```

### Error: "no Python 3.x interpreter found"

```bash
# Make sure you're in the virtual environment:
.venv\Scripts\activate

# Or explicitly use the venv Python:
.venv\Scripts\python.exe --version
```

### Error: "cargo build failed"

**Don't use `cargo build` directly!** Use `maturin build` instead:

```bash
maturin build --release
```

Maturin is the proper tool for building Rust Python extensions.

### Build is very slow

This is normal - first build takes 5-10 minutes. Subsequent builds are faster due to caching.

---

## Files You Need in Your Git Repository

Make sure these files are in your POSIX repository:

```
POSIX/
├── WINDOWS_SETUP_GUIDE.md           # This file
├── setup_libsql_build.bat           # Automated setup script
├── install_libunistd.bat            # libunistd installer
├── README.md                        # Project overview
├── QUICKSTART.md                    # Quick reference
├── docs/
│   └── BUILD_ENHANCED_LIBSQL.md    # Detailed build guide
└── .gitignore                       # Ignore deps/ directory

# Add to .gitignore:
deps/
.venv/
target/
*.whl
build.log
```

---

## Summary: New Computer Setup Checklist

- [ ] Install Rust (rustup)
- [ ] Install Git
- [ ] Install uv
- [ ] Clone POSIX repository
- [ ] Run `setup_libsql_build.bat`
- [ ] Clone/download libsql-experimental source
- [ ] Create virtual environment: `uv venv --python 3.13`
- [ ] Activate venv: `.venv\Scripts\activate`
- [ ] Install maturin: `uv pip install maturin`
- [ ] Set environment variables (POSIX_DIR, CFLAGS, etc.)
- [ ] Build: `maturin build --release`
- [ ] Install: `uv pip install target/wheels/*.whl`
- [ ] Test: `python -c "import libsql_experimental"`

**Total time: ~15-20 minutes** (including first build)

---

## Quick Command Reference

```bash
# Setup on new computer
git clone <posix-repo> POSIX
cd POSIX
setup_libsql_build.bat

# Build libsql-experimental
cd path\to\libsql_experimental
uv venv --python 3.13
.venv\Scripts\activate
uv pip install maturin
set POSIX_DIR=C:\path\to\POSIX
set CFLAGS=-I%POSIX_DIR%\deps\libunistd
set CXXFLAGS=-I%POSIX_DIR%\deps\libunistd
set BINDGEN_EXTRA_CLANG_ARGS=-I%POSIX_DIR%\deps\libunistd
maturin build --release
uv pip install target/wheels/*.whl
python -c "import libsql_experimental"
```

---

## Resources

- **POSIX Compatibility**: This repository
- **libsql**: https://github.com/tursodatabase/libsql
- **uv**: https://github.com/astral-sh/uv
- **maturin**: https://github.com/PyO3/maturin
- **Rust**: https://rustup.rs/

---

**Last Updated**: 2024-11-02
**Tested On**: Windows 11
**Status**: Production Ready ✅
