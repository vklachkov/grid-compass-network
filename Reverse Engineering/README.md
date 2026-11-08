# Reverse Engineering

This directory will collect research on GRiD Compass applications and the protocols they use to communicate with GRiD Server.

Publication plans are described in [issue #5](https://github.com/vklachkov/grid-compass-network/issues/5).

## Research Process

The server software and protocols are analyzed using IDA Pro.

GRiD applications are reverse-engineered using a [custom plugin and loader](https://github.com/vklachkov/ida-grid-os-loader) developed by my friend [@radioegor146](https://github.com/radioegor146).

[ida-pro-mcp](https://github.com/mrexodia/ida-pro-mcp) was used for LLM-assisted analysis.

Reverse engineering results are checked against program execution traces and surviving GRiD source code.
