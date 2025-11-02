# POSIX Compatibility for libsql on Windows

This repository contains everything needed to build **libsql-experimental** on Windows.

## 🚀 Quick Start (New Computer)

```bash
# 1. Prerequisites: Install Rust, Git, and uv
winget install Rustlang.Rustup
winget install Git.Git
powershell -c "irm https://astral.sh/uv/install.ps1 | iex"

# 2. Clone this repository
git clone <your-repo-url> POSIX
cd POSIX

# 3. Run automated setup
setup_libsql_build.bat

# 4. Done! Now build libsql-experimental (see WINDOWS_SETUP_GUIDE.md)
```

## 📚 Documentation

| Document | Purpose |
|----------|---------|
| [WINDOWS_SETUP_GUIDE.md](WINDOWS_SETUP_GUIDE.md) | **START HERE** - Complete setup guide for new computers |
| [QUICKSTART.md](QUICKSTART.md) | 5-minute quick reference |
| [BUILD_ENHANCED_LIBSQL.md](docs/BUILD_ENHANCED_LIBSQL.md) | Detailed build documentation |
| [RESEARCH_SUMMARY.md](RESEARCH_SUMMARY.md) | Technical background |

## ✨ What This Repository Does

- Provides POSIX compatibility layer (libunistd) for Windows
- Automated setup scripts for building libsql-experimental
- Complete documentation and troubleshooting guides
- Works with Python 3.8+

## 🎯 Key Files

- `setup_libsql_build.bat` - Main setup script
- `install_libunistd.bat` - Install POSIX compatibility
- `WINDOWS_SETUP_GUIDE.md` - Complete guide for new computers

## ⚡ Features

✅ No external dependencies (except Rust, Git, uv)
✅ Automated setup in ~5 minutes
✅ Cross-platform Python wheels (once built)
✅ Production-ready

## 🆘 Need Help?

1. Read [WINDOWS_SETUP_GUIDE.md](WINDOWS_SETUP_GUIDE.md)
2. Check [QUICKSTART.md](QUICKSTART.md)
3. See [Troubleshooting](WINDOWS_SETUP_GUIDE.md#troubleshooting)

## 📦 What Gets Installed

When you run `setup_libsql_build.bat`:

- Downloads libunistd (POSIX compatibility headers)
- Sets up environment variables
- Creates build helper scripts
- Verifies all prerequisites

**Nothing is installed globally** - everything stays in the POSIX directory.

## 🔧 Building libsql-experimental

After setup, to build libsql-experimental:

```bash
cd path\to\libsql_experimental
uv venv --python 3.13
.venv\Scripts\activate
uv pip install maturin

# Set POSIX environment
set POSIX_DIR=C:\path\to\POSIX
set CFLAGS=-I%POSIX_DIR%\deps\libunistd
set CXXFLAGS=-I%POSIX_DIR%\deps\libunistd
set BINDGEN_EXTRA_CLANG_ARGS=-I%POSIX_DIR%\deps\libunistd

# Build and install
maturin build --release
uv pip install target/wheels/*.whl

# Test
python -c "import libsql_experimental; print('Success!')"
```

## 📋 Requirements

- Windows 10/11
- Rust 1.70+
- Git
- uv (Python package manager)

## 🤝 Contributing

This is a utility repository for building libsql on Windows. For libsql issues, see:
https://github.com/tursodatabase/libsql

## 📄 License

MIT - See [LICENSE.md](LICENSE.md)

---

**Last Updated**: 2024-11-02
**Status**: Production Ready ✅
**Tested**: Windows 11, Python 3.13.7
