# Copilot Instructions for mctp-lib

## Project Overview

This is a **platform-agnostic MCTP (Management Component Transport Protocol) stack** implemented in Rust for the OpenPRoT project. The crate provides a bare-metal routing layer designed for embedded systems without std support.


## Project State & Development Strategy
**Current Status:** Early-stage project with minimal codebase (README + LICENSE only)

When adding code, expect to create:
- `Cargo.toml` with appropriate dependencies for embedded/no_std environments
- `src/lib.rs` as the main library entry point
- MCTP protocol implementation modules (packet parsing, routing, message handling)
- Platform-specific transport bindings (I2C, PCIe, USB, etc.)
- Error handling that works in both std and no_std environments

## Architecture Principles

### Protocol Implementation
- **Standards Compliance**: Follow DMTF MCTP specifications precisely
- **Transport Agnostic**: Core protocol separate from transport implementations
- **Embedded-First**: Design for resource-constrained environments (no_std support)
- **Security Focus**: Implement proper validation, bounds checking, and secure defaults


## Development Guidelines

### Rust-Specific Conventions
- **Use `#![no_std]` by default** with optional std feature flag

### Testing Strategy
- **Unit tests** for packet parsing and protocol logic
- **Integration tests** with mock transports
- **Embedded testing** on actual hardware when available

### Dependencies Management
- **Minimal dependencies** - justify each addition
- **Embedded-friendly crates only** (check no_std compatibility)
- **Avoid async** unless specifically needed (prefer blocking APIs)


Focus on creating clean, well-tested abstractions that can be easily integrated into larger firmware systems while maintaining security and reliability standards.

## Pull Request Review Checklist

- [ ] Code is completely panic-free (no unwrap/expect/panic/indexing)
- [ ] All fallible operations return Result or Option
- [ ] Integer operations use checked/saturating/wrapping methods where needed
- [ ] Array/slice access uses get() or pattern matching, not direct indexing
- [ ] Interior mutability uses try_borrow/try_borrow_mut, not borrow/borrow_mut
- [ ] Interior mutability requires justification (RefCell/Cell can cause runtime panics)
- [ ] Error cases are well documented and handled appropriately
- [ ] Tests verify error handling paths, not just happy paths
- [ ] No unsafe code added (crate enforces `#![deny(unsafe_code)]`)

## Quick Reference: Forbidden Patterns

| Forbidden Pattern | Required Alternative |
|-------------------|----------------------|
| `value.unwrap()` | `match value { Some(v) => v, None => return Err(...) }` |
| `result.expect("msg")` | `match result { Ok(v) => v, Err(e) => return Err(e.into()) }` |
| `collection[index]` | `collection.get(index).ok_or(Error::OutOfBounds)?` |
| `a + b` (integers) | `a.checked_add(b).ok_or(Error::Overflow)?` |
| `refcell.borrow()` | `refcell.try_borrow().map_err(\|_\| Error::BorrowConflict)?` |
| `refcell.borrow_mut()` | `refcell.try_borrow_mut().map_err(\|_\| Error::BorrowConflict)?` |

## File Organization

- `src/lib.rs`: Single-file crate with Router implementation and tests
- `Cargo.toml`: Minimal dependencies, Edition 2024
- No examples or additional modules currently

When extending this crate, follow the existing patterns of const generic parameterization and handle-based resource management.