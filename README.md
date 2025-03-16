# mcedit: Model Context Protocol File Editor

*⚠️  This project is experimental. Features may change without notice. Use with caution! ⚠️*

mcedit is a powerful command-line tool that enables AI assistants to edit files and analyze projects through the Model Context Protocol (MCP). It provides a secure bridge between AI models and your local filesystem.

## 🎮 Demo

See mcedit in action with Claude Desktop:

![mcedit Demo with Claude Desktop](.github/images/mcedit-demo.gif)

- Reading and writing any text file
- Analyzing project structures
- Searching and modifying file content
- Creating and managing files and directories
- Applying suggested edits from AI models

## 🎉 Latest Release

The first stable release of mcedit (v0.1.1) is now available on Crates.io! You can easily install it using Cargo:

```bash
cargo install mcedit
```

## Features

- 📝 **Universal File Editing**
  Works with any text-based files in any programming language or format.

- 📄 **MCP Server Capabilities**
  Runs as a Model Context Protocol server, allowing AI assistants to safely access and modify your files.

- 🔍 **Project Analysis**
  Analyzes project structure to provide AI assistants with better context for suggestions.

- ⚡️ **Blazing Fast**
  High-speed processing powered by the Rust ecosystem.

- 🔄 **Diff Generation**
  Creates and applies diffs to visualize and control changes.

- 🛡️ **Built-in Safety**
  Automatic backups before modifications and strict path validation for security.

## Installation

### From Source
```bash
# Clone the repository
git clone https://github.com/yourusername/mcedit
cd mcedit

# Build and install
cargo install --path .
```

### From Crates.io
```bash
cargo install mcedit
```

## Requirements

- Rust (edition 2021)
- Claude Desktop or any MCP-compatible AI assistant

# Zed File Context Server

This extension provides a Model Context Server for file operations, for use with the Zed AI assistant.

It adds several slash commands to the Assistant Panel to help you work with files and analyze projects.

## Configuration

To use the extension, you can optionally set a project directory in your Zed `settings.json`:

```json
{
  "context_servers": {
    "file-context-server": {
      "settings": {
        "project_directory": "/path/to/your/project"
      }
    }
  }
}
```

If no project directory is specified, the current working directory will be used.

## Usage

```bash
$ mcedit --help
✨ A CLI tool for smart file editing with AI assistance through the Model Context Protocol (MCP).

Usage: mcedit [OPTIONS] [COMMAND]

Commands:
  mcp       Launch mcedit as an MCP server
  edit      Edit a file with the given content
  list      List files in the project
  analyze   Analyze the project structure
  search    Search for text in project files
  help      Print this message or the help of the given subcommand(s)

Options:
  -c, --config <PATH>    Path to the configuration file
  -d, --dir <PATH>       Project directory to work with
  -V, --version          Print version
  -h, --help             Print help
```

### Integrating with Claude Desktop

To use mcedit with Claude Desktop:

1. If you haven't already, install mcedit:
```bash
cargo install mcedit
```

2. Find the path to your installed mcedit executable:
```bash
which mcedit
```

3. Add the following configuration to `~/Library/Application\ Support/Claude/claude_desktop_config.json`:

```json
{
    "mcpServers": {
        "mcedit": {
            "command": "/path/to/your/mcedit",  // Replace with the actual path from step 2
            "args": ["mcp"],
            "env": {
                "HOME": "/Users/yourusername",  // Replace with your username
                "PATH": "/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin",
                "PROJECT_DIR": "/path/to/your/project"  // Optional: specify your project directory
            }
        }
    }
}
```

4. Restart Claude Desktop and enable the mcedit tool.

5. mcedit will automatically create a sample README.md file in `~/project` if no project exists, ensuring Claude can start working with files right away.

## Logs and Troubleshooting

The mcedit server logs are available at:
```
~/Library/Logs/Claude/mcp-server-mcedit.log
```

## Environment Variables

- `PROJECT_DIR`: Set this to specify your project directory. If not set, mcedit will use the directory provided by command line arguments, configuration files, or fall back to `~/project`. You can also change the project directory at runtime using the `change_directory` tool.
- `MCEDIT_LOG_LEVEL`: Set to `debug`, `info`, `warn`, or `error` to control logging verbosity.

## Security Considerations

When using mcedit, please be aware of the following security considerations:

- mcedit creates automatic backups before modifying files
- Path validation prevents access to files outside the specified project directory
- Review code changes suggested by AI before applying them
- Sensitive information in your files might be accessible to AI assistants

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## Attribution

This project is derived from [tfmcp](https://github.com/nwiizo/tfmcp), originally developed by [nwiizo](https://github.com/nwiizo), under the MIT License.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
