# Connecting the GRiD Compass to the Network

A project to reimplement GRiD Server and reverse-engineer the built-in modem of the world's first laptop.

![GRiD Compass connected to the server](./Screenshot.jpg)

## Server Status

> [!WARNING]
> The server is an experimental, incomplete reimplementation intended for protocol research rather than production use.

The new server implements the GRiDLink transport, VIPC framing, a subset of VFS, and the GRiDMail workflows needed to list, read, and send messages, including attachments. Authentication, multi-user mail routing, most server applications, and full protocol compatibility are not implemented yet.

## Modem Reverse Engineering

The GRiD Compass modem drivers for MS-DOS have been reverse-engineered. This work also led to a [MAME pull request for HLE emulation of the internal modem](https://github.com/mamedev/mame/pull/15627).

No original modem schematic or technical documentation is currently available, so some hardware behavior remains uncertain.

## Reverse-Engineering Process

The modem was reverse-engineered manually from its MS-DOS and GRiD-OS drivers.

The server software is being analyzed with an LLM through [ida-pro-mcp](https://github.com/mrexodia/ida-pro-mcp) and the project-specific [ida-grid-os-loader](https://github.com/vklachkov/ida-grid-os-loader), with runtime traces and recovered GRiD sources used to verify the results.
