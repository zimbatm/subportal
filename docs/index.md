# subportal documentation

This documentation is organized using the [Diataxis](https://diataxis.fr/)
framework.

## Tutorials

Learning-oriented guides that walk you through a complete experience.

- [Getting started](tutorials/getting-started.md) -- set up subportal
  end-to-end and forward your first URL

## How-to guides

Task-oriented instructions for specific goals.

- [SSH setup](howto/ssh-setup.md) -- configure SSH reverse forwarding for
  subportal
- [NixOS / home-manager / system-manager setup](howto/nixos-setup.md) --
  declarative configuration with Nix modules
- [Manual installation](howto/manual-install.md) -- build from source and
  install without Nix
- [Troubleshooting](howto/troubleshooting.md) -- diagnose and fix common
  problems

## Reference

Technical descriptions of interfaces and configuration.

- [Protocol](reference/protocol.md) -- Varlink wire protocol, methods, and
  errors
- [CLI](reference/cli.md) -- command-line interface for all binaries
- [Nix modules](reference/nix-modules.md) -- NixOS, home-manager, and
  system-manager module options

## Explanation

Background and design rationale.

- [Architecture](explanation/architecture.md) -- how the components fit
  together
- [Security model](explanation/security.md) -- threat model, access control,
  and trust boundaries
