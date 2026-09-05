# Contributing Guide

Thank you for your interest in contributing to **vcd30player**! We welcome all forms of contributions, including bug reports, feature suggestions, documentation enhancements, and pull requests.

---

## 🛠️ Development Prerequisites

1. **Rust Toolchain**: Rust 1.80+ (2024 Edition, recommended: `rustup update stable`).
2. **C Compiler**:
   - Windows: MSVC or MinGW-w64
   - Linux: GCC or Clang
   - macOS: Apple Clang
3. **Git Submodules**:
   The project depends on `vendor/pl_mpeg` as a Git submodule. When cloning the repository, please make sure submodules are checked out recursively:
   ```bash
   git clone --recurse-submodules https://github.com/NimitzDEV/vcd30player.git
   cd vcd30player

   # If cloned without --recurse-submodules, initialize them via:
   git submodule update --init --recursive
   ```

---

## 🧪 Testing & Quality Assurance

Before submitting changes or opening a pull request, please make sure all tests pass locally:

```bash
# 1. Run all unit and integration tests (includes 47 self-contained tests)
cargo test

# 2. Run static analysis (Clippy)
cargo clippy

# 3. Check code formatting
cargo fmt -- --check
```

---

## 📝 Commit Message Guidelines

We follow the [Conventional Commits](https://www.conventionalcommits.org/) specification:

```text
<type>(<scope>): <description>
```

Common `<type>` prefixes include:
- `feat`: A new feature or capability
- `fix`: A bug fix
- `docs`: Documentation updates
- `style`: Formatting, missing semicolons, etc. (no functional code changes)
- `refactor`: Refactoring code without adding features or fixing bugs
- `test`: Adding or updating test cases
- `chore`: Tooling, build scripts, dependencies, or CI updates

---

## 🔀 Pull Request Workflow

1. Fork the repository and create a new feature branch from `master` (e.g. `feature/my-feature` or `fix/issue-description`).
2. Implement your changes with clean, well-tested code.
3. Ensure all tests pass with `cargo test`.
4. Submit a Pull Request describing the context, implementation details, and verification steps.
